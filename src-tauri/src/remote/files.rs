//! Remote listing, download (provider read streams) and device ejection.

use std::{
    fs,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use crate::plugins::provider_registry::FsOperation;

use super::{
    config::{
        connection_for_path, ensure_provider_access, local_for, provider_config, rel_string,
        safe_child_name,
    },
    store::{load_stored, StoredConnection},
    sync::{load_meta, save_meta},
};

#[derive(Clone)]
pub(crate) struct RemoteFile {
    pub(crate) name: String,
    pub(crate) is_dir: bool,
    pub(crate) size: u64,
    pub(crate) modified: u64,
    pub(crate) node_id: String,
    pub(crate) revision: String,
    /// Node-level restriction from the provider (`allowedOps`); `None` means
    /// the node did not narrow the plugin's declared capabilities.
    pub(crate) allowed_ops: Option<Vec<String>>,
    /// The provider marked this node read-only regardless of its capabilities.
    pub(crate) readonly: bool,
}

/// Capabilities a mount node really exposes: the plugin's declaration
/// intersected with the entry's own `allowedOps`. Core never grants more than
/// either layer allows.
fn effective_ops(plugin_id: &str, allowed: Option<&[String]>) -> Vec<FsOperation> {
    let declared = crate::plugins::provider_registry::operations(plugin_id).unwrap_or_default();
    FsOperation::ALL
        .into_iter()
        .filter(|operation| declared.contains(operation))
        .filter(|operation| {
            allowed.is_none_or(|list| {
                list.iter()
                    .any(|name| FsOperation::parse(name) == Some(*operation))
            })
        })
        .collect()
}

/// UI projection of a node's capabilities. `readonly` mirrors the protocol:
/// a node without write/rename/remove/mkdir support cannot be modified.
fn entry_capabilities(
    plugin_id: &str,
    allowed: Option<&[String]>,
    explicit_readonly: bool,
) -> (bool, Vec<String>) {
    let operations = effective_ops(plugin_id, allowed);
    let writable = operations.iter().any(|operation| {
        matches!(
            operation,
            FsOperation::Write | FsOperation::Rename | FsOperation::Remove | FsOperation::Mkdir
        )
    });
    (
        explicit_readonly || !writable,
        operations
            .into_iter()
            .map(|operation| operation.as_str().to_string())
            .collect(),
    )
}

fn parse_allowed_ops(entry: &serde_json::Value) -> Option<Vec<String>> {
    entry
        .get("allowedOps")
        .and_then(serde_json::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(serde_json::Value::as_str)
                .filter(|name| FsOperation::parse(name).is_some())
                .map(str::to_owned)
                .collect()
        })
}

pub(crate) fn list_remote(conn: &StoredConnection, rel: &str) -> Result<Vec<RemoteFile>, String> {
    let plugin_id = ensure_provider_access(conn)?;
    let config = provider_config(conn);
    let metadata = load_meta(conn);
    let node_id = metadata
        .files
        .get(rel)
        .map(|m| m.node_id.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(rel);
    let mut files = Vec::new();
    let mut cursor: Option<String> = None;
    let mut page_count = 0usize;
    loop {
        page_count += 1;
        if page_count > 1000 {
            return Err("provider sent too many pages".into());
        }
        let page = crate::plugins::host::global().invoke(
            &plugin_id,
            "fs.list",
            serde_json::json!({
                "mountId": conn.mount_id,
                "connection": config,
                "nodeId": node_id,
                "cursor": cursor.as_deref().unwrap_or("0"),
                "limit": 500
            }),
        )?;
        let entries = page
            .get("entries")
            .and_then(serde_json::Value::as_array)
            .ok_or("provider response missing entries")?;
        if entries.len() > 500 || files.len() + entries.len() > 100_000 {
            return Err("provider exceeded listing limit".into());
        }
        for entry in entries {
            let name = entry
                .get("name")
                .and_then(serde_json::Value::as_str)
                .ok_or("provider entry missing name")?;
            if safe_child_name(name).is_none() {
                return Err("provider returned an unsafe child name".into());
            }
            let kind = entry
                .get("kind")
                .and_then(serde_json::Value::as_str)
                .ok_or("provider entry missing kind")?;
            if kind != "directory" && kind != "file" {
                return Err("provider returned an unknown node kind".into());
            }
            let node_id = entry
                .get("nodeId")
                .and_then(serde_json::Value::as_str)
                .ok_or("provider entry missing nodeId")?;
            files.push(RemoteFile {
                node_id: node_id.into(),
                revision: entry
                    .get("revision")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .into(),
                allowed_ops: parse_allowed_ops(entry),
                readonly: entry
                    .get("readonly")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
                name: name.into(),
                is_dir: kind == "directory",
                size: entry
                    .get("size")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or_default(),
                modified: entry
                    .get("modified")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or_default(),
            });
        }
        let next = page
            .get("nextCursor")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        if next.is_none() {
            break;
        }
        if next == cursor || files.len() > 100_000 {
            return Err("provider pagination is invalid or too large".into());
        }
        cursor = next;
    }
    Ok(files)
}

/// Directory listing rendered from the plugin's VFS metadata. A virtual
/// remote item is represented by a stable logical mount path, not an empty
/// file on disk. Materialization happens only when the user opens/downloads.
pub async fn list_listing(
    path: &str,
    hidden: bool,
    ignores: &[String],
) -> Result<Option<crate::core::files::Listing>, String> {
    let Some((conn, relative)) = connection_for_path(path) else {
        return Ok(None);
    };
    let plugin_id = ensure_provider_access(&conn)?;
    let _guard = super::sync::lock(&conn).await;
    let rel = rel_string(&relative);
    let mut metadata = load_meta(&conn);
    let mut warnings = Vec::new();
    // Provider RPC and remote pagination are synchronous. Keep them off the
    // async runtime while preserving the per-connection metadata lock.
    let listing_conn = conn.clone();
    let listing_rel = rel.clone();
    let listed = tokio::task::spawn_blocking(move || list_remote(&listing_conn, &listing_rel))
        .await
        .map_err(|error| error.to_string())?;
    let files = match listed {
        Ok(files) => {
            remember_listing(&rel, &files, &mut metadata);
            save_meta(&conn, &metadata)?;
            files
        }
        Err(error) => {
            if !metadata.listed.contains(&rel) {
                return Err(error);
            }
            warnings.push(format!("离线目录：{error}"));
            cached_listing(&rel, &metadata)
        }
    };
    warnings.extend(
        metadata
            .errors
            .iter()
            .filter(|(key, _)| super::sync::in_tree(key, &rel))
            .map(|(key, error)| format!("{key}: {error}")),
    );
    let cached_dirs = super::cache::cached_dirs(&metadata);
    let filters = crate::core::ignores::IgnoreSet::new(ignores);
    let mut entries = Vec::new();
    for file in files {
        if (!hidden && file.name.starts_with('.')) || filters.is_ignored(Path::new(&file.name)) {
            continue;
        }
        let child = if relative.as_os_str().is_empty() {
            file.name.clone()
        } else {
            format!("{}/{}", rel_string(&relative), file.name)
        };
        let local = local_for(&conn, &child);
        let state = metadata.files.get(&child);
        let downloaded = state.is_some_and(|meta| meta.downloaded) && local.is_file();
        let has_cache = if file.is_dir {
            cached_dirs.contains(&child)
        } else {
            downloaded
        };
        let pinned = state.is_some_and(|meta| meta.pinned);
        let dirty = state.is_some_and(|meta| super::sync::dirty(&conn, &child, meta));
        // "Uploaded" is orthogonal to the sync mode, but only meaningful while
        // the entry still has a cache or a pin (an uncached entry has no copy).
        let uploaded = state.is_some_and(|meta| meta.uploaded && !meta.conflict)
            && !dirty
            && (has_cache || pinned);
        let (readonly, allowed_ops) =
            entry_capabilities(&plugin_id, file.allowed_ops.as_deref(), file.readonly);
        entries.push(crate::core::files::Entry {
            path: super::config::navigation_path(&conn, &child)?,
            name: file.name,
            is_dir: file.is_dir,
            is_symlink: false,
            size: file.size,
            modified: file.modified,
            created: 0,
            extension: local
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase(),
            mime: mime_guess::from_path(&local)
                .first_or_octet_stream()
                .to_string(),
            readonly,
            link_target: None,
            allowed_ops,
            tags: Vec::new(),
            has_cache,
            pinned,
            uploaded,
            sync: Some(
                state
                    .map(|m| {
                        if m.conflict {
                            "conflict"
                        } else if dirty {
                            "pending"
                        } else if m.pinned {
                            "pinned"
                        } else if has_cache {
                            "synced"
                        } else {
                            "cloud"
                        }
                    })
                    .unwrap_or("cloud")
                    .into(),
            ),
        });
    }
    entries.sort_by_cached_key(|entry| (!entry.is_dir, entry.name.to_lowercase()));
    let (mount_readonly, _) = entry_capabilities(&plugin_id, None, false);
    // Creating inside this directory needs the provider's `mkdir` and the
    // directory node's own stored `allowedOps`/`readonly`.
    let directory = metadata.files.get(&rel);
    let can_create = crate::plugins::provider_registry::supports(&plugin_id, FsOperation::Mkdir)
        && directory.is_none_or(|file| {
            !file.readonly
                && file.allowed_ops.as_deref().is_none_or(|ops| {
                    ops.is_empty() || ops.iter().any(|op| op == FsOperation::Mkdir.as_str())
                })
        });
    Ok(Some(crate::core::files::Listing {
        read_only: mount_readonly,
        can_create,
        virtual_volume: true,
        path: super::config::navigation_path(&conn, &rel)?,
        entries,
        warnings,
    }))
}

/// Plugins never receive absolute host paths, even for downloads.
/// Core opens a bounded stream, pulls offset chunks over IPC, and only Core
/// creates or commits the destination file. A failed or cancelled transfer
/// drops its staging file. No provider can write into the host filesystem.
pub type ProgressFn = dyn Fn(u64, Option<u64>) + Send + Sync;

#[derive(Clone, Default)]
pub struct TransferOptions {
    pub cancel: Option<Arc<AtomicBool>>,
    pub progress: Option<Arc<ProgressFn>>,
}

impl TransferOptions {
    pub(crate) fn cancelled(&self) -> bool {
        self.cancel
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
    }
    pub(crate) fn report(&self, done: u64, total: Option<u64>) {
        if let Some(progress) = &self.progress {
            progress(done, total);
        }
    }
}

pub(crate) fn fetch_remote(
    conn: &StoredConnection,
    rel: &str,
    dest: &Path,
    options: &TransferOptions,
) -> Result<u64, String> {
    use base64::Engine;
    use std::io::Write;

    const CHUNK: u64 = 256 * 1024;
    const MAX_TRANSFER: u64 = i64::MAX as u64;
    let plugin_id = ensure_provider_access(conn)?;
    if options.cancelled() {
        return Err(crate::core::operations::CANCELLED.into());
    }
    super::config::check_cache_path(conn, dest)?;
    let parent = dest.parent().ok_or("invalid download destination")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut staging = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    let opened = crate::plugins::host::global().invoke(
        &plugin_id,
        "fs.openRead",
        serde_json::json!({
            "mountId": conn.mount_id,
            "connection": provider_config(conn),
            "nodeId": rel
        }),
    )?;
    let stream_id = opened
        .get("streamId")
        .and_then(serde_json::Value::as_str)
        .ok_or("provider returned no read stream")?
        .to_string();
    let total = opened
        .get("size")
        .and_then(serde_json::Value::as_u64)
        .filter(|size| *size <= MAX_TRANSFER);
    let mut offset = 0u64;
    struct ReadLease<'a> {
        plugin: &'a str,
        mount_id: &'a str,
        stream: &'a str,
    }
    impl Drop for ReadLease<'_> {
        fn drop(&mut self) {
            let _ = crate::plugins::host::global().invoke(
                self.plugin,
                "fs.closeRead",
                serde_json::json!({"mountId": self.mount_id, "streamId": self.stream}),
            );
        }
    }
    let _lease = ReadLease {
        plugin: &plugin_id,
        mount_id: &conn.mount_id,
        stream: &stream_id,
    };
    loop {
        if options.cancelled() {
            return Err(crate::core::operations::CANCELLED.into());
        }
        if offset >= MAX_TRANSFER {
            return Err("remote file exceeds maximum allowed transfer size".into());
        }
        let response = match crate::plugins::host::global().invoke(
            &plugin_id,
            "fs.readStream",
            serde_json::json!({
                "mountId": conn.mount_id,
                "streamId": &stream_id,
                "offset": offset,
                "maxBytes": CHUNK
            }),
        ) {
            Ok(response) => response,
            Err(error) => {
                return Err(error);
            }
        };
        let encoded = response
            .get("base64")
            .and_then(serde_json::Value::as_str)
            .ok_or("provider returned no byte data")?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|_| "provider returned invalid base64")?;
        if bytes.len() > CHUNK as usize || offset + bytes.len() as u64 > MAX_TRANSFER {
            return Err("provider exceeded the transfer frame size".into());
        }
        let eof = response
            .get("eof")
            .and_then(serde_json::Value::as_bool)
            .ok_or("provider returned no EOF flag")?;
        if bytes.is_empty() && !eof {
            return Err("provider returned an empty nonfinal chunk".into());
        }
        staging.write_all(&bytes).map_err(|e| e.to_string())?;
        offset += bytes.len() as u64;
        options.report(offset, total);
        if eof {
            break;
        }
    }
    if total.is_some_and(|size| offset != size) {
        return Err("remote stream length does not match advertised size".into());
    }
    staging.as_file().sync_all().map_err(|e| e.to_string())?;
    staging.persist(dest).map_err(|e| e.error.to_string())?;
    Ok(offset)
}

/// Native provider-backed listing. Protocol I/O is owned by the installed,
/// explicitly approved plugin; Core only resolves an existing connection and
/// forwards a page request. This path never creates placeholder files.
pub async fn provider_list(
    kind: &str,
    connection_id: &str,
    relative: &str,
    cursor: Option<&str>,
    limit: usize,
) -> Result<serde_json::Value, String> {
    if relative.starts_with('/')
        || relative.split('/').any(|segment| {
            segment == ".." || segment == "." || segment.contains('\\') || segment.contains('\0')
        })
    {
        return Err("invalid remote relative path".into());
    }
    let conn = load_stored()
        .into_iter()
        .find(|c| c.id == connection_id && c.kind == kind)
        .ok_or("connection not found")?;
    // The manifest and actual user-granted permission gate every request.
    // No built-in provider is trusted implicitly, including the official ones.
    let plugin_id = ensure_provider_access(&conn)?;
    let config = provider_config(&conn);
    let metadata = load_meta(&conn);
    let node_id = metadata
        .files
        .get(relative)
        .map(|m| m.node_id.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(relative);
    crate::plugins::host::global().invoke(
        &plugin_id,
        "fs.list",
        serde_json::json!({
            "mountId": conn.mount_id,
            "connection": config,
            "nodeId": node_id,
            "cursor": cursor.unwrap_or("0"),
            "limit": limit.clamp(1, 500)
        }),
    )
}

pub async fn download(path: &str, options: &TransferOptions) -> Result<String, String> {
    let Some((conn, _)) = connection_for_path(path) else {
        crate::plugins::mounts::reject_local(path)?;
        return Ok(path.into());
    };
    let _guard = super::sync::lock(&conn).await;
    download_inner(path, options)
}

pub(crate) fn download_inner(path: &str, options: &TransferOptions) -> Result<String, String> {
    let Some((conn, rel)) = connection_for_path(path) else {
        crate::plugins::mounts::reject_local(path)?;
        return Ok(path.into());
    };
    ensure_provider_access(&conn)?;
    let rel = rel_string(&rel);
    let mut meta = load_meta(&conn);
    let local = local_for(&conn, &rel);
    if local.is_file() && meta.files.get(&rel).is_some_and(|m| m.downloaded) {
        return Ok(local.to_string_lossy().into_owned());
    }
    if local.is_file() {
        return Err(crate::i18n::t("本地已有未登记的文件，已保留；请先备份或处理同步冲突").into());
    }
    if !meta.files.contains_key(&rel) {
        let parent = rel.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
        let listing = list_remote(&conn, parent)?;
        remember_listing(parent, &listing, &mut meta);
    }
    if meta.files.get(&rel).is_some_and(|f| f.revision.is_empty()) {
        let file = &meta.files[&rel];
        if let Ok(info) = crate::plugins::host::global().invoke(
            &ensure_provider_access(&conn)?,
            "fs.stat",
            serde_json::json!({
                "mountId": conn.mount_id,
                "connection": provider_config(&conn),
                "nodeId": if file.node_id.is_empty() { &rel } else { &file.node_id }
            }),
        ) {
            meta.files.get_mut(&rel).unwrap().revision =
                info["revision"].as_str().unwrap_or("").into();
        }
    }
    let entry = meta.files.get(&rel).ok_or("远程文件不存在")?.clone();
    if entry.is_dir {
        return Err(crate::i18n::t("不能作为文件打开文件夹").into());
    }
    let node = if entry.node_id.is_empty() {
        &rel
    } else {
        &entry.node_id
    };
    fetch_remote(&conn, node, &local, options)?;
    let file = meta.files.get_mut(&rel).unwrap();
    file.downloaded = true;
    file.uploaded = false;
    file.baseline = super::sync::fingerprint(&local)?;
    file.local_stamp = super::sync::stamp(&local)?;
    save_meta(&conn, &meta)?;
    Ok(local.to_string_lossy().into_owned())
}

pub async fn download_dir(path: &str, options: &TransferOptions) -> Result<(), String> {
    let Some((conn, _)) = connection_for_path(path) else {
        return Ok(());
    };
    let _guard = super::sync::lock(&conn).await;
    download_dir_inner(path, options)
}

pub(crate) fn download_dir_inner(path: &str, options: &TransferOptions) -> Result<(), String> {
    let Some((conn, rel)) = connection_for_path(path) else {
        return Ok(());
    };
    let mut pending = vec![rel_string(&rel)];
    while let Some(current) = pending.pop() {
        if options.cancelled() {
            return Err(crate::core::operations::CANCELLED.into());
        }
        let mut meta = load_meta(&conn);
        let files = list_remote(&conn, &current)?;
        remember_listing(&current, &files, &mut meta);
        fs::create_dir_all(local_for(&conn, &current)).map_err(|e| e.to_string())?;
        save_meta(&conn, &meta)?;
        for file in files {
            let child = child_path(&current, &file.name);
            if meta.files.get(&child).is_some_and(|f| f.excluded) {
                continue;
            }
            if file.is_dir {
                pending.push(child);
            } else {
                download_inner(&local_for(&conn, &child).to_string_lossy(), options)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn child_path(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.into()
    } else {
        format!("{parent}/{name}")
    }
}

pub(crate) fn remember_listing(
    parent: &str,
    files: &[RemoteFile],
    meta: &mut super::sync::RemoteMeta,
) {
    meta.listed.insert(parent.into());
    let inherited = meta
        .files
        .iter()
        .any(|(key, file)| file.pinned && file.is_dir && super::sync::in_tree(parent, key));
    // A cached file's revision remains its download baseline. Listing must not
    // silently advance that baseline before reconciliation checks for conflicts.
    for file in files {
        let child = child_path(parent, &file.name);
        let old = meta.files.entry(child).or_default();
        if !old.downloaded {
            old.size = file.size;
            old.modified = file.modified;
            old.revision = file.revision.clone();
        }
        old.node_id = file.node_id.clone();
        old.allowed_ops = file.allowed_ops.clone();
        old.readonly = file.readonly;
        old.is_dir = file.is_dir;
        if !old.excluded {
            old.pinned |= inherited;
        }
    }
}

fn cached_listing(parent: &str, meta: &super::sync::RemoteMeta) -> Vec<RemoteFile> {
    meta.files
        .iter()
        .filter_map(|(key, m)| {
            let (dir, name) = key.rsplit_once('/').unwrap_or(("", key));
            (dir == parent).then(|| RemoteFile {
                name: name.into(),
                is_dir: m.is_dir,
                size: m.size,
                modified: m.modified,
                node_id: m.node_id.clone(),
                revision: m.revision.clone(),
                allowed_ops: m.allowed_ops.clone(),
                readonly: m.readonly,
            })
        })
        .collect()
}

pub async fn ensure_downloaded(path: &str, options: &TransferOptions) -> Result<String, String> {
    if connection_for_path(path).is_some() {
        download(path, options).await
    } else {
        crate::plugins::mounts::reject_local(path)?;
        Ok(path.into())
    }
}

pub fn eject(path: &str) -> Result<(), String> {
    crate::plugins::mounts::reject_local(path)?;
    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("diskutil")
            .args(["eject", path])
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            return Ok(());
        }
        return Err(crate::i18n::t("无法弹出该设备").into());
    }
    #[cfg(target_os = "linux")]
    {
        let status = std::process::Command::new("gio")
            .args(["mount", "-u", path])
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            return Ok(());
        }
        let status = std::process::Command::new("udisksctl")
            .args(["unmount", "-b", path])
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            return Ok(());
        }
        return Err(crate::i18n::t("无法卸载该设备").into());
    }
    #[cfg(target_os = "windows")]
    {
        let drive = path.chars().next().ok_or(crate::i18n::t("无效设备"))?;
        let script = format!(
            "$d=(New-Object -ComObject Shell.Application).Namespace(17).ParseName('{drive}:');$d.InvokeVerb('Eject')"
        );
        let mut command = std::process::Command::new("powershell");
        command.args(["-NoProfile", "-Command", &script]);
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
        let status = command.status().map_err(|e| e.to_string())?;
        if status.success() {
            return Ok(());
        }
        return Err(crate::i18n::t("无法弹出该设备").into());
    }
    #[allow(unreachable_code)]
    Err(crate::i18n::t("当前平台不支持弹出设备").into())
}

/// Metadata remains usable before a file has ever been downloaded.
pub fn stat(path: &str) -> Result<crate::core::files::Entry, String> {
    let (conn, relative) = connection_for_path(path).ok_or("MountNotFound: connection removed")?;
    let plugin_id = ensure_provider_access(&conn)?;
    let rel = rel_string(&relative);
    let mut meta = load_meta(&conn);
    if !rel.is_empty() && !meta.files.contains_key(&rel) {
        let parent = rel.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
        remember_listing(parent, &list_remote(&conn, parent)?, &mut meta);
        save_meta(&conn, &meta)?;
    }
    let file = meta.files.get(&rel);
    if !rel.is_empty() && file.is_none() {
        return Err("remote node not found".into());
    }
    let pinned = file.is_some_and(|entry| entry.pinned);
    let dirty = file.is_some_and(|entry| super::sync::dirty(&conn, &rel, entry));
    let has_cache = file.is_some_and(|entry| {
        if entry.is_dir {
            super::cache::subtree_has_cache(&meta, &rel)
        } else {
            entry.downloaded && local_for(&conn, &rel).is_file()
        }
    });
    let uploaded = file.is_some_and(|entry| entry.uploaded && !entry.conflict)
        && !dirty
        && (has_cache || pinned);
    let name = relative
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| conn.name.clone());
    let (readonly, allowed_ops) = file
        .map(|entry| entry_capabilities(&plugin_id, entry.allowed_ops.as_deref(), entry.readonly))
        .unwrap_or_else(|| entry_capabilities(&plugin_id, None, false));
    Ok(crate::core::files::Entry {
        path: super::config::navigation_path(&conn, &rel)?,
        name,
        is_dir: file.is_none_or(|m| m.is_dir),
        is_symlink: false,
        size: file.map(|m| m.size).unwrap_or(0),
        modified: file.map(|m| m.modified).unwrap_or(0),
        created: 0,
        extension: relative
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        mime: mime_guess::from_path(&relative)
            .first_or_octet_stream()
            .to_string(),
        readonly,
        link_target: None,
        allowed_ops,
        tags: vec![],
        has_cache,
        pinned,
        uploaded,
        sync: Some(
            if file.is_some_and(|entry| entry.conflict) {
                "conflict"
            } else if dirty {
                "pending"
            } else if pinned {
                "pinned"
            } else if has_cache {
                "synced"
            } else {
                "cloud"
            }
            .into(),
        ),
    })
}
/// OS operations receive only a checked materialized cache path.
pub fn cached_path(path: &str) -> Result<String, String> {
    let (conn, rel) = connection_for_path(path).ok_or("MountNotFound: connection removed")?;
    ensure_provider_access(&conn)?;
    let local = local_for(&conn, &rel_string(&rel));
    super::config::check_cache_path(&conn, &local)?;
    Ok(local.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::super::sync::{RemoteMeta, RemoteMetaFile};
    use super::*;
    fn remote(name: &str) -> RemoteFile {
        RemoteFile {
            name: name.into(),
            is_dir: false,
            size: 12,
            modified: 0,
            node_id: "stable-id".into(),
            revision: "new-version".into(),
            allowed_ops: Some(vec!["read".into(), "write".into()]),
            readonly: false,
        }
    }
    #[test]
    fn offline_listing_survives_metadata_round_trip() {
        let mut meta = RemoteMeta::default();
        remember_listing("folder", &[remote("file.txt")], &mut meta);
        let recovered: RemoteMeta =
            serde_json::from_slice(&serde_json::to_vec(&meta).unwrap()).unwrap();
        assert!(recovered.listed.contains("folder"));
        let files = cached_listing("folder", &recovered);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].node_id, "stable-id");
        assert_eq!(
            files[0].allowed_ops.as_deref(),
            Some(&["read".to_string(), "write".to_string()][..])
        );
        assert!(cached_listing("other", &recovered).is_empty());
    }
    #[test]
    fn readonly_intersects_declared_and_node_capabilities() {
        let plugin = "org.example.capability-test";
        crate::plugins::provider_registry::register(
            plugin,
            &serde_json::json!({"fs":{"operations":["list","stat","read","write","remove"]}}),
        );
        assert!(!entry_capabilities(plugin, None, false).0);
        let read_only = vec!["read".to_string(), "stat".to_string()];
        assert!(entry_capabilities(plugin, Some(&read_only), false).0);
        let writable = vec!["read".to_string(), "write".to_string(), "mkdir".to_string()];
        assert!(!entry_capabilities(plugin, Some(&writable), false).0);
        // A node cannot expose an operation the plugin never declared.
        let undeclared = vec!["rename".to_string()];
        assert!(entry_capabilities(plugin, Some(&undeclared), false).0);
        crate::plugins::provider_registry::forget(plugin);
        assert!(entry_capabilities(plugin, None, false).0);
    }
    #[test]
    fn listing_cannot_advance_download_revision() {
        let mut meta = RemoteMeta::default();
        meta.files.insert(
            "file.txt".into(),
            RemoteMetaFile {
                downloaded: true,
                revision: "download-version".into(),
                baseline: "local-hash".into(),
                ..Default::default()
            },
        );
        remember_listing("", &[remote("file.txt")], &mut meta);
        assert_eq!(meta.files["file.txt"].revision, "download-version");
        assert_eq!(meta.files["file.txt"].baseline, "local-hash");
    }
    #[test]
    fn folder_pin_inheritance_respects_explicit_eviction() {
        let mut meta = RemoteMeta::default();
        meta.files.insert(
            "folder".into(),
            RemoteMetaFile {
                pinned: true,
                is_dir: true,
                ..Default::default()
            },
        );
        remember_listing("folder", &[remote("a"), remote("b")], &mut meta);
        assert!(meta.files["folder/a"].pinned);
        let b = meta.files.get_mut("folder/b").unwrap();
        b.excluded = true;
        b.pinned = false;
        remember_listing("folder", &[remote("b")], &mut meta);
        assert!(!meta.files["folder/b"].pinned);
    }
}
