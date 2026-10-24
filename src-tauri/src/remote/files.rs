//! Remote listing, download (provider read streams) and device ejection.

use std::{
    fs,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use super::{
    config::{
        connection_for_path, ensure_provider_access, local_for, provider_config, rel_string,
        safe_child_name,
    },
    store::{load_stored, StoredConnection},
    sync::{load_meta, save_meta, RemoteMetaFile},
};

#[derive(Clone)]
struct RemoteFile {
    name: String,
    is_dir: bool,
    size: u64,
    modified: u64,
}

async fn list_remote(conn: &StoredConnection, rel: &str) -> Result<Vec<RemoteFile>, String> {
    let plugin_id = ensure_provider_access(conn)?;
    let config = provider_config(conn);
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
                "directoryId": rel,
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
    let files = list_remote(&conn, &rel_string(&relative)).await?;
    let metadata = load_meta(&conn);
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
            sync: Some(if downloaded { "synced" } else { "cloud" }.into()),
        });
    }
    entries.sort_by_cached_key(|entry| (!entry.is_dir, entry.name.to_lowercase()));
    Ok(Some(crate::files::Listing {
        read_only: true,
        virtual_volume: true,
        path: path.into(),
        entries,
        warnings: Vec::new(),
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
    fn cancelled(&self) -> bool {
        self.cancel
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
    }
    fn report(&self, done: u64, total: Option<u64>) {
        if let Some(progress) = &self.progress {
            progress(done, total);
        }
    }
}

async fn fetch_remote(
    conn: &StoredConnection,
    rel: &str,
    dest: &Path,
    options: &TransferOptions,
) -> Result<u64, String> {
    use base64::Engine;
    use std::io::Write;

    const CHUNK: u64 = 256 * 1024;
    const MAX_TRANSFER: u64 = 16 * 1024 * 1024 * 1024;
    let plugin_id = ensure_provider_access(conn)?;
    if options.cancelled() {
        return Err(crate::operations::CANCELLED.into());
    }
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
        .filter(|size| *size > 0 && *size <= MAX_TRANSFER);
    let mut offset = 0u64;
    let close = |stream: &str| {
        let _ = crate::plugin_host::global().invoke(
            &plugin_id,
            "fs.closeRead",
            serde_json::json!({ "streamId": stream }),
        );
    };
    loop {
        if options.cancelled() {
            close(&stream_id);
            return Err(crate::operations::CANCELLED.into());
        }
        if offset >= MAX_TRANSFER {
            close(&stream_id);
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
                close(&stream_id);
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
            close(&stream_id);
            return Err("provider exceeded the transfer frame size".into());
        }
        let eof = response
            .get("eof")
            .and_then(serde_json::Value::as_bool)
            .ok_or("provider returned no EOF flag")?;
        if bytes.is_empty() && !eof {
            close(&stream_id);
            return Err("provider returned an empty nonfinal chunk".into());
        }
        staging.write_all(&bytes).map_err(|e| e.to_string())?;
        offset += bytes.len() as u64;
        options.report(offset, total);
        if eof {
            break;
        }
    }
    close(&stream_id);
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
    let Some((conn, rel)) = connection_for_path(path) else {
        return Ok(path.into());
    };
    let rel = rel_string(&rel);
    let mut meta = load_meta(&conn);
    let local = local_for(&conn, &rel);
    if local.is_file() && meta.files.get(&rel).is_some_and(|m| m.downloaded) {
        return Ok(local.to_string_lossy().into_owned());
    }
    if let Some(parent) = local.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let temp = local.with_extension(format!(
        "fiex-download-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default()
    ));
    let size = match fetch_remote(&conn, &rel, &temp, options).await {
        Ok(size) => size,
        Err(error) => {
            let _ = fs::remove_file(&temp);
            return Err(error);
        }
    };
    fs::rename(&temp, &local).map_err(|e| e.to_string())?;
    let modified = meta.files.get(&rel).map(|m| m.modified).unwrap_or_default();
    meta.files.insert(
        rel,
        RemoteMetaFile {
            size,
            modified,
            is_dir: false,
            downloaded: true,
        },
    );
    save_meta(&conn, &meta)?;
    Ok(local.to_string_lossy().into_owned())
}

pub async fn download_dir(path: &str, options: &TransferOptions) -> Result<(), String> {
    let Some((conn, rel)) = connection_for_path(path) else {
        return Ok(());
    };
    let rel = rel_string(&rel);
    let mut meta = load_meta(&conn);
    let mut pending = vec![rel];
    while let Some(current) = pending.pop() {
        if options.cancelled() {
            return Err(crate::operations::CANCELLED.into());
        }
        meta.files.insert(
            current.clone(),
            RemoteMetaFile {
                size: 0,
                modified: 0,
                is_dir: true,
                downloaded: true,
            },
        );
        let files = list_remote(&conn, &current).await?;
        let mut dirs = Vec::new();
        for file in files {
            let Some(name) = safe_child_name(&file.name) else {
                continue;
            };
            let child = if current.is_empty() {
                name.to_string()
            } else {
                format!("{current}/{name}")
            };
            let local = local_for(&conn, &child);
            if file.is_dir {
                fs::create_dir_all(&local).map_err(|e| e.to_string())?;
            } else if let Some(parent) = local.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            if file.is_dir {
                dirs.push(child);
            } else {
                let local = local_for(&conn, &child);
                let cached =
                    meta.files.get(&child).is_some_and(|m| m.downloaded) && local.is_file();
                if !cached {
                    let temp = local.with_extension("fiex-download");
                    let size = match fetch_remote(&conn, &child, &temp, options).await {
                        Ok(size) => size,
                        Err(error) => {
                            let _ = fs::remove_file(&temp);
                            return Err(error);
                        }
                    };
                    fs::rename(&temp, &local).map_err(|e| e.to_string())?;
                    meta.files.insert(
                        child,
                        RemoteMetaFile {
                            size,
                            modified: file.modified,
                            is_dir: false,
                            downloaded: true,
                        },
                    );
                }
            }
        }
        pending.extend(dirs);
    }
    save_meta(&conn, &meta)
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
