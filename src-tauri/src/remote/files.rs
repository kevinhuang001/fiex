//! Remote listing, download (provider read streams) and device ejection.

use std::{
    fs,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

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
}

pub(crate) fn list_remote(conn: &StoredConnection, rel: &str) -> Result<Vec<RemoteFile>, String> {
    let plugin_id = ensure_provider_access(conn)?;
    let config = provider_config(conn);
    let metadata = load_meta(conn);
    let directory_id = metadata
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
        let page = crate::plugin_host::global().invoke(
            &plugin_id,
            "fs.list",
            serde_json::json!({
                "connection": config,
                "directoryId": directory_id,
                "cursor": cursor.as_deref().unwrap_or("0"),
                "limit": 500
            }),
        )?;
        let entries = page
            .get("entries")
            .and_then(serde_json::Value::as_array)
            .ok_or("provider response missing entries")?;
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
            files.push(RemoteFile {
                node_id: entry
                    .get("nodeId")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .into(),
                revision: entry
                    .get("revision")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .into(),
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
) -> Result<Option<crate::files::Listing>, String> {
    let Some((conn, relative)) = connection_for_path(path) else {
        return Ok(None);
    };
    let _guard = super::sync::lock(&conn).await;
    let rel = rel_string(&relative);
    let mut metadata = load_meta(&conn);
    let mut warnings = Vec::new();
    let files = match list_remote(&conn, &rel) {
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
    let filters = crate::ignores::IgnoreSet::new(ignores);
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
        let downloaded = metadata
            .files
            .get(&child)
            .is_some_and(|meta| meta.downloaded)
            && local.is_file();
        entries.push(crate::files::Entry {
            path: local.to_string_lossy().into_owned(),
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
            readonly: true,
            link_target: None,
            tags: Vec::new(),
            sync: Some(
                metadata
                    .files
                    .get(&child)
                    .map(|m| {
                        if m.conflict {
                            "conflict"
                        } else if super::sync::dirty(&conn, &child, m) {
                            "pending"
                        } else if m.pinned {
                            "pinned"
                        } else if downloaded {
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
    Ok(Some(crate::files::Listing {
        read_only: true,
        virtual_volume: true,
        path: path.into(),
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
        return Err(crate::operations::CANCELLED.into());
    }
    super::config::check_cache_path(conn, dest)?;
    let parent = dest.parent().ok_or("invalid download destination")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut staging = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    let opened = crate::plugin_host::global().invoke(
        &plugin_id,
        "fs.openRead",
        serde_json::json!({"connection": provider_config(conn), "nodeId": rel}),
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
        stream: &'a str,
    }
    impl Drop for ReadLease<'_> {
        fn drop(&mut self) {
            let _ = crate::plugin_host::global().invoke(
                self.plugin,
                "fs.closeRead",
                serde_json::json!({"streamId":self.stream}),
            );
        }
    }
    let _lease = ReadLease {
        plugin: &plugin_id,
        stream: &stream_id,
    };
    loop {
        if options.cancelled() {
            return Err(crate::operations::CANCELLED.into());
        }
        if offset >= MAX_TRANSFER {
            return Err("remote file exceeds maximum allowed transfer size".into());
        }
        let response = match crate::plugin_host::global().invoke(
            &plugin_id,
            "fs.readStream",
            serde_json::json!({
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
    crate::plugin_host::global().invoke(
        &plugin_id,
        "fs.list",
        serde_json::json!({
            "connection": config,
            "directoryId": relative,
            "cursor": cursor.unwrap_or("0"),
            "limit": limit.clamp(1, 500)
        }),
    )
}

pub async fn download(path: &str, options: &TransferOptions) -> Result<String, String> {
    let Some((conn, _)) = connection_for_path(path) else {
        return Ok(path.into());
    };
    let _guard = super::sync::lock(&conn).await;
    download_inner(path, options)
}

pub(crate) fn download_inner(path: &str, options: &TransferOptions) -> Result<String, String> {
    let Some((conn, rel)) = connection_for_path(path) else {
        return Ok(path.into());
    };
    let rel = rel_string(&rel);
    let mut meta = load_meta(&conn);
    let local = local_for(&conn, &rel);
    if local.is_file() && meta.files.get(&rel).is_some_and(|m| m.downloaded) {
        return Ok(local.to_string_lossy().into_owned());
    }
    if local.is_file() {
        return Err("本地已有未登记的文件，已保留；请先备份或处理同步冲突".into());
    }
    if !meta.files.contains_key(&rel) {
        let parent = rel.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
        let listing = list_remote(&conn, parent)?;
        remember_listing(parent, &listing, &mut meta);
    }
    let entry = meta.files.get(&rel).ok_or("远程文件不存在")?.clone();
    if entry.is_dir {
        return Err("不能作为文件打开文件夹".into());
    }
    let node = if entry.node_id.is_empty() {
        &rel
    } else {
        &entry.node_id
    };
    fetch_remote(&conn, node, &local, options)?;
    let file = meta.files.get_mut(&rel).unwrap();
    file.downloaded = true;
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
            return Err(crate::operations::CANCELLED.into());
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
            })
        })
        .collect()
}

pub async fn ensure_downloaded(path: &str, options: &TransferOptions) -> Result<String, String> {
    if connection_for_path(path).is_some() {
        download(path, options).await
    } else {
        Ok(path.into())
    }
}

pub fn eject(path: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("diskutil")
            .args(["eject", path])
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            return Ok(());
        }
        return Err("无法弹出该设备".into());
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
        return Err("无法卸载该设备".into());
    }
    #[cfg(target_os = "windows")]
    {
        let drive = path.chars().next().ok_or("无效设备")?;
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
        return Err("无法弹出该设备".into());
    }
    #[allow(unreachable_code)]
    Err("当前平台不支持弹出设备".into())
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
        assert!(cached_listing("other", &recovered).is_empty());
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
