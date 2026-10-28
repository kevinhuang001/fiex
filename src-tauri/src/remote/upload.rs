//! Uploading local (or materialized) trees into a provider-backed connection.
//!
//! The provider receives ordinary `fs.beginWrite` calls: `parentId` + `name`
//! for new nodes and `nodeId` + `ifRevision` for replacements. Core keeps the
//! metadata snapshot in step so the explorer shows the new entries without a
//! full re-list.

use super::{
    config::{connection_for_path, ensure_provider_access, navigation_path, rel_string},
    files::{child_path, list_remote, RemoteFile},
    store::StoredConnection,
    sync::{load_meta, save_meta, RemoteMetaFile},
};
use crate::{
    core::{
        ignores::IgnoreSet,
        operations::{check_cancelled, OperationResult},
        transfers::{Action, Conflict, FileVersion, Resolution},
    },
    plugins::resolvers::fs::{mount_for_path, FsEntry, Mount, WriteRequest},
};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

const CHUNK: usize = 256 * 1024;

/// One top-level item to upload. `local` is a real file after remote/VFS
/// sources have been materialized by the caller.
pub(crate) struct Source {
    pub(crate) local: PathBuf,
    pub(crate) name: String,
}

struct FileTask {
    local: PathBuf,
    rel: String,
    name: String,
    size: u64,
    replace: Option<RemoteFile>,
}

struct DirTask {
    rel: String,
    name: String,
    /// Present when a same-named file must be removed before mkdir.
    existed: Option<RemoteFile>,
    /// Present when an existing directory is merged into.
    node: Option<String>,
}

fn unique_name(name: &str, taken: &HashSet<String>) -> String {
    if !taken.contains(name) {
        return name.to_string();
    }
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.to_string());
    let ext = path
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();
    for index in 2..10_000 {
        let candidate = format!("{stem} ({index}){ext}");
        if !taken.contains(&candidate) {
            return candidate;
        }
    }
    name.to_string()
}

fn version(local: &Path) -> Result<(u64, u64, bool), String> {
    let meta = fs::symlink_metadata(local).map_err(|error| error.to_string())?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    Ok((meta.len(), modified, meta.is_dir()))
}

fn conflict_key(local: &Path, destination: &str, source_size: u64, dest_size: u64) -> String {
    format!(
        "{}\0{}\0{}\0{}",
        local.to_string_lossy(),
        destination,
        source_size,
        dest_size
    )
}

fn action_for(key: &str, resolutions: &[Resolution], keep_both: bool) -> Option<Action> {
    if keep_both {
        return Some(Action::KeepBoth);
    }
    resolutions
        .iter()
        .find(|resolution| resolution.key == key)
        .map(|resolution| resolution.action)
}

#[allow(clippy::too_many_arguments)]
fn walk(
    conn: &StoredConnection,
    mount: &Mount,
    ignore: &IgnoreSet,
    local: &Path,
    dest_parent: &str,
    name: &str,
    existing: &HashMap<String, RemoteFile>,
    resolutions: &[Resolution],
    keep_both: bool,
    cancel: &AtomicBool,
    tasks: &mut Vec<FileTask>,
    dirs: &mut Vec<DirTask>,
    conflicts: &mut Vec<Conflict>,
    skipped: &mut usize,
    reserved: &mut HashSet<String>,
) -> Result<(), String> {
    check_cancelled(cancel)?;
    if ignore.is_ignored(Path::new(name)) {
        *skipped += 1;
        return Ok(());
    }
    let meta = fs::symlink_metadata(local).map_err(|error| error.to_string())?;
    if meta.file_type().is_symlink() || !(meta.is_file() || meta.is_dir()) {
        return Err(crate::i18n::t("暂不支持复制或移动符号链接及特殊文件").into());
    }
    let relation = child_path(dest_parent, name);
    let destination = navigation_path(conn, &relation)?;
    let existing = existing.get(name);
    let is_dir = meta.is_dir();
    let (source_size, source_modified, _) = version(local)?;

    if is_dir {
        if let Some(item) = existing {
            if !item.is_dir {
                let key = conflict_key(local, &destination, source_size, item.size);
                match action_for(&key, resolutions, keep_both) {
                    Some(Action::Skip) => {
                        *skipped += 1;
                        return Ok(());
                    }
                    Some(Action::KeepBoth) => {
                        let unique = unique_name(name, reserved);
                        reserved.insert(unique.clone());
                        let relation = child_path(dest_parent, &unique);
                        dirs.push(DirTask {
                            rel: relation.clone(),
                            name: unique,
                            existed: None,
                            node: None,
                        });
                        // Children of a freshly kept directory cannot collide.
                        copy_children(
                            conn,
                            mount,
                            ignore,
                            local,
                            &relation,
                            &HashMap::new(),
                            resolutions,
                            keep_both,
                            cancel,
                            tasks,
                            dirs,
                            conflicts,
                            skipped,
                            reserved,
                        )?;
                        return Ok(());
                    }
                    Some(Action::Replace) => {
                        dirs.push(DirTask {
                            rel: relation.clone(),
                            name: name.to_string(),
                            existed: Some(item.clone()),
                            node: None,
                        });
                        let children = list_remote(conn, &relation)?
                            .into_iter()
                            .map(|child| (child.name.clone(), child))
                            .collect();
                        copy_children(
                            conn,
                            mount,
                            ignore,
                            local,
                            &relation,
                            &children,
                            resolutions,
                            keep_both,
                            cancel,
                            tasks,
                            dirs,
                            conflicts,
                            skipped,
                            reserved,
                        )?;
                        return Ok(());
                    }
                    None => conflicts.push(Conflict {
                        key,
                        source: FileVersion {
                            path: local.to_string_lossy().into_owned(),
                            size: source_size,
                            modified: source_modified,
                            is_dir: true,
                        },
                        destination: FileVersion {
                            path: destination,
                            size: item.size,
                            modified: item.modified,
                            is_dir: item.is_dir,
                        },
                    }),
                }
                return Ok(());
            }
            // Existing directory: merge into it and remember its provider node
            // so files created below it receive the right parent.
            dirs.push(DirTask {
                rel: relation.clone(),
                name: name.to_string(),
                existed: None,
                node: Some(item.node_id.clone()),
            });
            let children = list_remote(conn, &relation)?
                .into_iter()
                .map(|child| (child.name.clone(), child))
                .collect();
            copy_children(
                conn,
                mount,
                ignore,
                local,
                &relation,
                &children,
                resolutions,
                keep_both,
                cancel,
                tasks,
                dirs,
                conflicts,
                skipped,
                reserved,
            )?;
            return Ok(());
        }
        dirs.push(DirTask {
            rel: relation.clone(),
            name: name.to_string(),
            existed: None,
            node: None,
        });
        copy_children(
            conn,
            mount,
            ignore,
            local,
            &relation,
            &HashMap::new(),
            resolutions,
            keep_both,
            cancel,
            tasks,
            dirs,
            conflicts,
            skipped,
            reserved,
        )?;
        return Ok(());
    }

    // A file.
    let Some(item) = existing else {
        tasks.push(FileTask {
            local: local.to_path_buf(),
            rel: relation,
            name: name.to_string(),
            size: source_size,
            replace: None,
        });
        return Ok(());
    };
    let key = conflict_key(local, &destination, source_size, item.size);
    match action_for(&key, resolutions, keep_both) {
        Some(Action::Skip) => *skipped += 1,
        Some(Action::KeepBoth) => {
            let unique = unique_name(name, reserved);
            reserved.insert(unique.clone());
            tasks.push(FileTask {
                local: local.to_path_buf(),
                rel: child_path(dest_parent, &unique),
                name: unique,
                size: source_size,
                replace: None,
            });
        }
        Some(Action::Replace) => tasks.push(FileTask {
            local: local.to_path_buf(),
            rel: relation,
            name: name.to_string(),
            size: source_size,
            replace: Some(item.clone()),
        }),
        None => conflicts.push(Conflict {
            key,
            source: FileVersion {
                path: local.to_string_lossy().into_owned(),
                size: source_size,
                modified: source_modified,
                is_dir: false,
            },
            destination: FileVersion {
                path: destination,
                size: item.size,
                modified: item.modified,
                is_dir: item.is_dir,
            },
        }),
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn copy_children(
    conn: &StoredConnection,
    mount: &Mount,
    ignore: &IgnoreSet,
    local: &Path,
    dest_parent: &str,
    existing: &HashMap<String, RemoteFile>,
    resolutions: &[Resolution],
    keep_both: bool,
    cancel: &AtomicBool,
    tasks: &mut Vec<FileTask>,
    dirs: &mut Vec<DirTask>,
    conflicts: &mut Vec<Conflict>,
    skipped: &mut usize,
    reserved: &mut HashSet<String>,
) -> Result<(), String> {
    for entry in fs::read_dir(local).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        walk(
            conn,
            mount,
            ignore,
            &entry.path(),
            dest_parent,
            &name,
            existing,
            resolutions,
            keep_both,
            cancel,
            tasks,
            dirs,
            conflicts,
            skipped,
            reserved,
        )?;
    }
    Ok(())
}

/// Upload the sources into the connection owning `destination`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn upload(
    destination: &str,
    sources: &[Source],
    keep_both: bool,
    ignores: &[String],
    resolutions: &[Resolution],
    progress: &(dyn Fn(u64, u64, Option<&str>) + Sync),
    cancel: &AtomicBool,
) -> Result<OperationResult, String> {
    let (conn, relative) =
        connection_for_path(destination).ok_or("MountNotFound: connection removed")?;
    ensure_provider_access(&conn)?;
    let _guard = futures::executor::block_on(super::sync::lock(&conn));
    let mount = mount_for_path(destination)?;
    let dest_rel = rel_string(&relative);
    let meta = load_meta(&conn);
    let parent_node = meta
        .files
        .get(&dest_rel)
        .map(|file| file.node_id.clone())
        .filter(|node| !node.is_empty())
        .unwrap_or_else(|| dest_rel.clone());
    let existing: HashMap<String, RemoteFile> = list_remote(&conn, &dest_rel)?
        .into_iter()
        .map(|file| (file.name.clone(), file))
        .collect();

    let mut tasks = Vec::new();
    let mut dirs = Vec::new();
    let mut conflicts = Vec::new();
    let mut skipped = 0usize;
    let mut reserved: HashSet<String> = existing.keys().cloned().collect();
    let ignore = IgnoreSet::new(ignores);
    for source in sources {
        walk(
            &conn,
            &mount,
            &ignore,
            &source.local,
            &dest_rel,
            &source.name,
            &existing,
            resolutions,
            keep_both,
            cancel,
            &mut tasks,
            &mut dirs,
            &mut conflicts,
            &mut skipped,
            &mut reserved,
        )?;
    }
    if !conflicts.is_empty() {
        return Ok(OperationResult {
            paths: Vec::new(),
            errors: Vec::new(),
            skipped,
            conflicts,
        });
    }

    let mut meta = load_meta(&conn);
    let mut nodes: HashMap<String, String> = HashMap::new();
    nodes.insert(dest_rel.clone(), parent_node.clone());
    let mut created: Vec<String> = Vec::new();

    for dir in &dirs {
        check_cancelled(cancel)?;
        let parent_rel = dir.rel.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
        let parent = nodes
            .get(parent_rel)
            .cloned()
            .unwrap_or_else(|| parent_rel.to_string());
        if let Some(node) = &dir.node {
            nodes.insert(dir.rel.clone(), node.clone());
            continue;
        }
        if let Some(existing) = &dir.existed {
            mount.remove(&existing.node_id)?;
        }
        let entry = mount.mkdir(&parent, &dir.name)?;
        nodes.insert(dir.rel.clone(), entry.node_id.clone());
        let mut file = RemoteMetaFile {
            is_dir: true,
            ..RemoteMetaFile::default()
        };
        remember(&entry, &mut file);
        meta.files.insert(dir.rel.clone(), file);
        created.push(navigation_path(&conn, &dir.rel)?);
    }

    let total = tasks.len() as u64;
    let mut errors = Vec::new();
    for (index, task) in tasks.iter().enumerate() {
        check_cancelled(cancel)?;
        progress(index as u64 + 1, total, Some(&task.name));
        if let Err(error) = upload_file(&conn, &mount, task, &mut nodes, &mut meta) {
            errors.push(format!("{}: {error}", task.local.display()));
        } else {
            created.push(navigation_path(&conn, &task.rel)?);
        }
        save_meta(&conn, &meta)?;
    }
    save_meta(&conn, &meta)?;
    Ok(OperationResult {
        paths: created,
        errors,
        skipped,
        conflicts: Vec::new(),
    })
}

fn remember(entry: &FsEntry, file: &mut RemoteMetaFile) {
    file.node_id = entry.node_id.clone();
    file.revision = entry.revision.clone().unwrap_or_default();
    file.is_dir = entry.is_dir();
    file.size = entry.size.unwrap_or_default();
    file.modified = entry.modified.unwrap_or(0).max(0) as u64;
    if let Some(readonly) = entry.readonly {
        file.readonly = readonly;
    }
    if let Some(operations) = &entry.allowed_ops {
        file.allowed_ops = Some(
            operations
                .iter()
                .map(|operation| operation.as_str().to_string())
                .collect(),
        );
    }
}

fn upload_file(
    conn: &StoredConnection,
    mount: &Mount,
    task: &FileTask,
    nodes: &mut HashMap<String, String>,
    meta: &mut super::sync::RemoteMeta,
) -> Result<(), String> {
    let parent_rel = task.rel.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
    let parent = nodes
        .get(parent_rel)
        .cloned()
        .unwrap_or_else(|| parent_rel.to_string());
    let mut input = fs::File::open(&task.local).map_err(|error| error.to_string())?;
    let (node_id, revision) = match &task.replace {
        Some(existing) if !existing.revision.is_empty() => (
            Some(existing.node_id.clone()),
            Some(existing.revision.clone()),
        ),
        Some(existing) => {
            mount.remove(&existing.node_id)?;
            (None, None)
        }
        None => (None, None),
    };
    let mut stream = mount
        .begin_write(WriteRequest {
            node_id: node_id.as_deref(),
            parent_id: if node_id.is_some() {
                None
            } else {
                Some(&parent)
            },
            name: if node_id.is_some() {
                None
            } else {
                Some(&task.name)
            },
            size: Some(task.size),
            if_revision: revision.as_deref(),
        })
        .map_err(|error| error.to_string())?;
    let mut buffer = vec![0u8; CHUNK];
    loop {
        let read = input.read(&mut buffer).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        stream
            .write(&buffer[..read])
            .map_err(|error| error.to_string())?;
    }
    let committed = stream.commit().map_err(|error| error.to_string())?;
    let mut file = RemoteMetaFile {
        size: task.size,
        downloaded: false,
        uploaded: true,
        ..RemoteMetaFile::default()
    };
    if let Some(entry) = &committed {
        remember(entry, &mut file);
        nodes.insert(task.rel.clone(), entry.node_id.clone());
    } else if let Some(node) = node_id {
        nodes.insert(task.rel.clone(), node);
    }
    meta.files.insert(task.rel.clone(), file);
    let _ = conn;
    Ok(())
}
