//! Generic plugin-owned virtual filesystems.
//!
//! Explicit VFS extensions are routed through Core-owned view identities and the
//! shared [`crate::plugins::resolvers::fs`], so views use the same `fs.*` protocol as
//! connections. Views remain navigation identities; UI extensions never confer
//! provider eligibility.
use crate::core::files::{Entry, Listing};
use crate::plugins::provider_registry::FsOperation;
use crate::plugins::resolvers::fs::{FsEntry, FsKind, Mount, Source};
use serde_json::Value;

/// Stream reads are chunked so a large export never sits in memory.
const READ_CHUNK: usize = 256 * 1024;

/// Claim every Core URI, including invalid/revoked ones, before OS fallback.
pub fn handles(path: &str) -> bool {
    crate::plugins::mounts::handles(path)
}

fn resolve(
    path: &str,
) -> Result<(crate::plugins::mounts::Uri, crate::plugins::mounts::View), String> {
    let (uri, source) = crate::plugins::mounts::resolve(path)?;
    match source {
        crate::plugins::mounts::Source::View(view) => Ok((uri, view)),
        crate::plugins::mounts::Source::Connection(_) => {
            Err("connection request requires the Core connection adapter".into())
        }
    }
}
fn node(
    path: &str,
) -> Result<
    (
        crate::plugins::mounts::Uri,
        crate::plugins::mounts::View,
        String,
    ),
    String,
> {
    let (uri, mut view) = resolve(path)?;
    if view.node_id(&uri.relative)?.is_none() {
        // Address-bar/bookmark navigation may target an opaque node not visited
        // this session. Resolve only its ancestors, never enumerate the mount.
        let mut parent = String::new();
        for part in uri.relative.split('/') {
            let target = if parent.is_empty() {
                part.to_string()
            } else {
                format!("{parent}/{part}")
            };
            let current = crate::plugins::mounts::view(&uri.mount_id)?;
            if current.node_id(&target)?.is_none() {
                list(
                    &crate::plugins::mounts::uri(&uri.mount_id, &parent)?,
                    true,
                    &[],
                )?;
            }
            parent = target;
        }
        view = crate::plugins::mounts::view(&uri.mount_id)?;
    }
    let id = view.node_id(&uri.relative)?.ok_or("VFS node not found")?;
    Ok((uri, view, id))
}
/// The provider identity a view sends with every `fs.*` request.
fn mount_for(uri: &crate::plugins::mounts::Uri, view: &crate::plugins::mounts::View) -> Mount {
    Mount {
        mount_id: uri.mount_id.clone(),
        plugin_id: view.descriptor.plugin_id.clone(),
        source: Source::View {
            contribution_id: view.descriptor.contribution_id.clone(),
            view_context: view.descriptor.view_context.clone(),
        },
        scope: Some(view.scope.clone()),
    }
}
/// Whether the mount declares any operation that changes the tree.
fn writable(mount: &Mount) -> bool {
    [
        FsOperation::Write,
        FsOperation::Mkdir,
        FsOperation::Rename,
        FsOperation::Remove,
    ]
    .into_iter()
    .any(|operation| mount.supports(operation))
}
/// Child location of `name`, never trusting a provider-supplied path.
fn child(relative: &str, name: &str) -> Result<String, String> {
    crate::plugins::mounts::segment(name)?;
    Ok(if relative.is_empty() {
        name.to_string()
    } else {
        format!("{relative}/{name}")
    })
}
fn project_entry(
    mount: &Mount,
    uri: &crate::plugins::mounts::Uri,
    relative: &str,
    entry: FsEntry,
) -> Result<(Entry, String), String> {
    crate::plugins::mounts::segment(&entry.name)?;
    // The provider's node is authoritative, but its name must match the
    // location Core already bound it to.
    if !relative.is_empty() && relative.rsplit('/').next() != Some(entry.name.as_str()) {
        return Err("provider node name does not match its location".into());
    }
    let path = crate::plugins::mounts::uri(&uri.mount_id, relative)?;
    let allowed_ops = FsOperation::ALL
        .into_iter()
        .filter(|operation| mount.supports(*operation) && entry.allows(*operation))
        .map(|operation| operation.as_str().to_string())
        .collect::<Vec<_>>();
    let readonly = entry.readonly.unwrap_or(false)
        || ![
            FsOperation::Write,
            FsOperation::Mkdir,
            FsOperation::Rename,
            FsOperation::Remove,
        ]
        .into_iter()
        .any(|operation| mount.supports(operation) && entry.allows(operation));
    Ok((
        Entry {
            path,
            name: entry.name,
            is_dir: entry.kind == FsKind::Directory,
            is_symlink: entry.kind == FsKind::Symlink,
            size: entry.size.unwrap_or(0),
            modified: entry.modified.unwrap_or(0).max(0) as u64,
            created: 0,
            extension: entry.extension.unwrap_or_default(),
            mime: entry.mime.unwrap_or_default(),
            readonly,
            link_target: entry.link_target,
            tags: Vec::new(),
            has_cache: false,
            pinned: false,
            uploaded: false,
            sync: None,
            allowed_ops,
        },
        entry.node_id,
    ))
}
fn bound_entry(
    mount: &Mount,
    uri: &crate::plugins::mounts::Uri,
    relative: &str,
    entry: FsEntry,
) -> Result<Entry, String> {
    let (entry, id) = project_entry(mount, uri, relative, entry)?;
    crate::plugins::mounts::bind_node(&uri.mount_id, relative, &id)?;
    Ok(entry)
}
pub fn entry_from(value: Value) -> Result<Entry, String> {
    serde_json::from_value(value).map_err(|error| format!("插件返回了无效的目录项：{error}"))
}

pub fn list(path: &str, hidden: bool, ignores: &[String]) -> Result<Listing, String> {
    let (uri, view, node_id) = node(path)?;
    let mount = mount_for(&uri, &view);
    let listing = mount.list(&node_id, hidden, ignores)?;
    let mut entries = Vec::with_capacity(listing.entries.len());
    for item in listing.entries {
        let relative = child(&uri.relative, &item.name)?;
        entries.push(bound_entry(&mount, &uri, &relative, item)?);
    }
    // Creating inside a directory needs both the provider operation and the
    // directory node's own `allowedOps`. An unreadable node offers no creation.
    let can_create = mount.supports(FsOperation::Mkdir)
        && mount
            .stat(&node_id)
            .is_ok_and(|entry| entry.allows(FsOperation::Mkdir) && entry.readonly != Some(true));
    Ok(Listing {
        read_only: !writable(&mount),
        can_create,
        virtual_volume: true,
        path: crate::plugins::mounts::uri(&uri.mount_id, &uri.relative)?,
        entries,
        warnings: listing.warnings,
    })
}

pub fn stat(path: &str) -> Result<Entry, String> {
    let (uri, view, node_id) = node(path)?;
    let mount = mount_for(&uri, &view);
    let entry = mount.stat(&node_id)?;
    bound_entry(&mount, &uri, &uri.relative, entry)
}

/// Read at most `max_bytes` from the start of a file through a provider stream.
pub fn read_prefix(path: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
    let (uri, view, node_id) = node(path)?;
    let mount = mount_for(&uri, &view);
    let stream = mount.open_read(&node_id)?;
    let mut bytes = Vec::new();
    let mut offset = 0u64;
    while bytes.len() < max_bytes {
        let want = (max_bytes - bytes.len()).min(READ_CHUNK);
        let (chunk, eof) = stream.read_at(offset, want)?;
        if chunk.is_empty() {
            break;
        }
        offset += chunk.len() as u64;
        bytes.extend_from_slice(&chunk);
        if eof {
            break;
        }
    }
    Ok(bytes)
}

pub fn search(
    path: &str,
    query: &str,
    hidden: bool,
    ignores: &[String],
    limit: usize,
) -> Result<(Vec<Entry>, bool), String> {
    let (uri, view, node_id) = node(path)?;
    let mount = mount_for(&uri, &view);
    let (items, truncated) = mount.search(&node_id, query, hidden, ignores, limit)?;
    let entries = items
        .into_iter()
        .map(|item| {
            let relative = item
                .relative_path
                .clone()
                .ok_or("search entry missing relativePath")?;
            crate::plugins::mounts::uri(&uri.mount_id, &relative)?;
            if !uri.relative.is_empty() && !relative.starts_with(&format!("{}/", uri.relative)) {
                return Err("search entry escaped subtree".into());
            }
            bound_entry(&mount, &uri, &relative, item)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((entries, truncated))
}

/// Read-only exports for external applications. No tree snapshots or persistent
/// blob cache: temp directories live until process exit, independently of tabs,
/// so closing one tab cannot invalidate another consumer's export.
fn exports() -> &'static std::sync::Mutex<std::collections::HashMap<String, tempfile::TempDir>> {
    static EXPORTS: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, tempfile::TempDir>>,
    > = std::sync::OnceLock::new();
    EXPORTS.get_or_init(Default::default)
}
// On Windows, writable temp exports must be cleared before removal. This code
// is compiled only for Windows; its Unix permission semantics are irrelevant.
#[cfg_attr(windows, allow(clippy::permissions_set_readonly_false))]
pub fn cleanup_exports() {
    if let Ok(mut exports) = exports().lock() {
        #[cfg(windows)]
        for directory in exports.values() {
            if let Ok(entries) = std::fs::read_dir(directory.path()) {
                for entry in entries.flatten() {
                    if let Ok(metadata) = entry.metadata() {
                        let mut permissions = metadata.permissions();
                        permissions.set_readonly(false);
                        let _ = std::fs::set_permissions(entry.path(), permissions);
                    }
                }
            }
        }
        exports.clear();
    }
}
pub fn materialize(path: &str) -> Result<String, String> {
    let entry = stat(path)?; // rechecks the owner and scope even on export reuse
    if entry.is_dir {
        return Err("VFS directories cannot be opened as OS folders".into());
    }
    crate::plugins::mounts::segment(&entry.name)?;
    let mut exports = exports()
        .lock()
        .map_err(|_| "export registry unavailable")?;
    if let Some(directory) = exports.get(path) {
        return Ok(directory
            .path()
            .join(&entry.name)
            .to_string_lossy()
            .into_owned());
    }
    if exports.len() >= 64 {
        return Err("too many open VFS exports; restart Fiex to release temporary exports".into());
    }
    let (uri, view, node_id) = node(path)?;
    let mount = mount_for(&uri, &view);
    let stream = mount.open_read(&node_id)?;
    let directory = tempfile::tempdir().map_err(|e| e.to_string())?;
    let target = directory.path().join(&entry.name);
    let mut written = 0u64;
    {
        use std::io::Write;
        let mut file = std::fs::File::create(&target).map_err(|e| e.to_string())?;
        let mut offset = 0u64;
        loop {
            let (chunk, eof) = stream.read_at(offset, READ_CHUNK)?;
            if chunk.is_empty() {
                break;
            }
            file.write_all(&chunk).map_err(|e| e.to_string())?;
            offset += chunk.len() as u64;
            written += chunk.len() as u64;
            if eof {
                break;
            }
        }
        file.flush().map_err(|e| e.to_string())?;
    }
    if stream.size().is_some_and(|expected| expected != written) {
        return Err("VFS export is incomplete".into());
    }
    let mut permissions = std::fs::metadata(&target)
        .map_err(|e| e.to_string())?
        .permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&target, permissions).map_err(|e| e.to_string())?;
    let result = target.to_string_lossy().into_owned();
    exports.insert(path.into(), directory);
    Ok(result)
}

/// Create an empty file or a directory inside `path`, returning its URI.
pub fn create(path: &str, name: &str) -> Result<String, String> {
    let (uri, view, node_id) = node(path)?;
    let mount = mount_for(&uri, &view);
    let created = Some(mount.mkdir(&node_id, name)?);
    match created {
        Some(entry) => {
            let relative = child(&uri.relative, &entry.name)?;
            crate::plugins::mounts::bind_node(&uri.mount_id, &relative, &entry.node_id)?;
            crate::plugins::mounts::uri(&uri.mount_id, &relative)
        }
        None => crate::plugins::mounts::uri(&uri.mount_id, &child(&uri.relative, name)?),
    }
}

/// Rename the node at `path`, returning its new URI.
pub fn rename(path: &str, name: &str) -> Result<String, String> {
    let (uri, view, node_id) = node(path)?;
    let mount = mount_for(&uri, &view);
    let entry = mount.rename(&node_id, name, None)?;
    let parent = uri
        .relative
        .rsplit_once('/')
        .map_or("", |(parent, _)| parent);
    let relative = child(parent, &entry.name)?;
    crate::plugins::mounts::bind_node(&uri.mount_id, &relative, &entry.node_id)?;
    crate::plugins::mounts::uri(&uri.mount_id, &relative)
}

pub fn remove(path: &str) -> Result<(), String> {
    let (uri, view, node_id) = node(path)?;
    mount_for(&uri, &view).remove(&node_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn file_entry(name: &str) -> FsEntry {
        FsEntry {
            node_id: "opaque-node".into(),
            name: name.into(),
            kind: FsKind::File,
            size: Some(7),
            modified: Some(0),
            extension: Some("txt".into()),
            mime: Some("text/plain".into()),
            revision: None,
            readonly: None,
            link_target: None,
            allowed_ops: None,
            relative_path: None,
        }
    }
    fn mount(plugin_id: &str) -> Mount {
        Mount {
            mount_id: "owner-one".into(),
            plugin_id: plugin_id.into(),
            source: Source::View {
                contribution_id: "tree".into(),
                view_context: json!({}),
            },
            scope: None,
        }
    }

    #[test]
    fn provider_cannot_choose_a_different_mount_or_escape_with_a_name() {
        let location = crate::plugins::mounts::parse("fiex://mount/owner-one/folder").unwrap();
        let mount = mount("test.vfs.identity");
        let (projected, node) = project_entry(
            &mount,
            &location,
            "folder/report #%.txt",
            file_entry("report #%.txt"),
        )
        .unwrap();
        assert_eq!(node, "opaque-node");
        assert_eq!(
            projected.path,
            "fiex://mount/owner-one/folder/report%20%23%25.txt"
        );
        assert!(projected.readonly && projected.sync.is_none());
        assert!(project_entry(
            &mount,
            &location,
            "folder/../secret",
            file_entry("../secret")
        )
        .is_err());
        assert!(
            project_entry(&mount, &location, "other.txt", file_entry("report #%.txt")).is_err()
        );
    }

    #[test]
    fn capabilities_narrow_the_provider_and_mark_entries_readonly() {
        let plugin = "test.vfs.capabilities";
        crate::plugins::provider_registry::register(
            plugin,
            &json!({"fs":{"operations":["list","stat","read","write","rename","remove"]}}),
        );
        let location = crate::plugins::mounts::parse("fiex://mount/owner-one/folder").unwrap();
        let mount = mount(plugin);
        let mut writable = file_entry("a.txt");
        writable.allowed_ops = Some(vec![FsOperation::Read, FsOperation::Rename]);
        let (projected, _) = project_entry(&mount, &location, "folder/a.txt", writable).unwrap();
        assert_eq!(projected.allowed_ops, vec!["read", "rename"]);
        assert!(!projected.readonly);
        let mut read_only = file_entry("b.txt");
        read_only.allowed_ops = Some(vec![FsOperation::Read]);
        let (projected, _) = project_entry(&mount, &location, "folder/b.txt", read_only).unwrap();
        assert_eq!(projected.allowed_ops, vec!["read"]);
        assert!(projected.readonly);
        crate::plugins::provider_registry::forget(plugin);
    }

    #[test]
    fn child_uris_reject_traversal_and_keep_the_mount() {
        assert_eq!(child("folder", "a.txt").unwrap(), "folder/a.txt");
        assert_eq!(child("", "a.txt").unwrap(), "a.txt");
        assert!(child("folder", "../secret").is_err());
    }
}
