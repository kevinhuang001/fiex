use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use smb::ReadAt;
use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    str::FromStr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredConnection {
    id: String,
    name: String,
    kind: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    host: String,
    #[serde(default)]
    share: String,
    #[serde(default)]
    base: String,
    #[serde(default)]
    username: String,
    // Historical JSON may include a password; never serialize one again.
    #[serde(default, skip_serializing)]
    password: String,
    #[serde(default)]
    allow_invalid_certs: bool,
    #[serde(default)]
    port: Option<u16>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInput {
    pub id: Option<String>,
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub share: String,
    #[serde(default)]
    pub base: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub allow_invalid_certs: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub mount_path: String,
    pub host: String,
    pub url: String,
    pub share: String,
    pub base: String,
}

#[derive(Clone)]
struct RemoteFile {
    name: String,
    path: String,
    is_dir: bool,
    size: u64,
    modified: u64,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteMetaFile {
    size: u64,
    modified: u64,
    is_dir: bool,
    downloaded: bool,
}

#[derive(Default, Serialize, Deserialize)]
struct RemoteMeta {
    files: HashMap<String, RemoteMetaFile>,
}

fn fiex_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".fiex")
}

fn connections_path() -> PathBuf {
    fiex_dir().join("connections.json")
}

fn drives_dir() -> PathBuf {
    fiex_dir().join("drives")
}

fn meta_dir() -> PathBuf {
    fiex_dir().join("remote-meta")
}

fn load_stored() -> Vec<StoredConnection> {
    let mut items: Vec<StoredConnection> = fs::read(connections_path())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let mut migrated = false;
    for item in &mut items {
        if !item.password.is_empty()
            && crate::plugin_storage::secret_set(
                &format!("org.fiex.{}", item.kind),
                &format!("connection-{}", item.id),
                "password",
                &item.password,
            )
            .is_ok()
        {
            item.password.clear();
            migrated = true;
        }
    }
    // Commit sanitized legacy data before populating in-memory credentials.
    if migrated {
        let _ = save_stored(&items);
    }
    for item in &mut items {
        if item.password.is_empty() {
            item.password = crate::plugin_storage::secret_get(
                &format!("org.fiex.{}", item.kind),
                &format!("connection-{}", item.id),
                "password",
            )
            .unwrap_or_default();
        }
    }
    items
}

fn save_stored(items: &[StoredConnection]) -> Result<(), String> {
    fs::create_dir_all(fiex_dir()).map_err(|e| e.to_string())?;
    let path = connections_path();
    fs::write(
        &path,
        serde_json::to_vec_pretty(items).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn mount_path(conn: &StoredConnection) -> PathBuf {
    drives_dir().join(&conn.id)
}

fn meta_path(conn: &StoredConnection) -> PathBuf {
    meta_dir().join(format!("{}.json", conn.id))
}

fn load_meta(conn: &StoredConnection) -> RemoteMeta {
    fs::read(meta_path(conn))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn save_meta(conn: &StoredConnection, meta: &RemoteMeta) -> Result<(), String> {
    fs::create_dir_all(meta_dir()).map_err(|e| e.to_string())?;
    fs::write(
        meta_path(conn),
        serde_json::to_vec(meta).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

/// Explicitly pass credentials only to the selected, authorized provider
/// subprocess; the connection registry itself never serializes passwords.
fn provider_config(conn: &StoredConnection) -> serde_json::Value {
    serde_json::json!({
        "url": conn.url,
        "host": conn.host,
        "port": conn.port,
        "share": conn.share,
        "base": conn.base,
        "username": conn.username,
        "password": conn.password,
        "allowInvalidCerts": conn.allow_invalid_certs
    })
}

fn public(conn: &StoredConnection) -> ConnectionInfo {
    let mount = mount_path(conn);
    let _ = fs::create_dir_all(&mount);
    ConnectionInfo {
        id: conn.id.clone(),
        name: conn.name.clone(),
        kind: conn.kind.clone(),
        mount_path: mount.to_string_lossy().into_owned(),
        host: conn.host.clone(),
        url: conn.url.clone(),
        share: conn.share.clone(),
        base: conn.base.clone(),
    }
}

pub fn list_connections() -> Vec<ConnectionInfo> {
    load_stored().iter().map(public).collect()
}

pub fn save_connection(input: ConnectionInput) -> Result<ConnectionInfo, String> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err("请输入连接名称".into());
    }
    let kind = input.kind.trim();
    if kind != "webdav" && kind != "smb" {
        return Err("不支持的连接类型".into());
    }
    if kind == "webdav" {
        let url = input.url.trim();
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err("WebDAV 地址必须以 http:// 或 https:// 开头".into());
        }
    } else if input.host.trim().is_empty() || input.share.trim().is_empty() {
        return Err("请填写 SMB 主机和共享名".into());
    }
    if input.port == Some(0) {
        return Err("请输入有效的端口".into());
    }
    let mut items = load_stored();
    let existing = input
        .id
        .as_deref()
        .and_then(|id| items.iter().position(|c| c.id == id));
    let id = if let Some(index) = existing {
        items[index].name = name.into();
        items[index].kind = kind.into();
        items[index].url = input.url.trim().into();
        items[index].host = input.host.trim().into();
        items[index].port = input.port;
        items[index].share = input.share.trim().into();
        items[index].base = input.base.trim().into();
        items[index].username = input.username.trim().into();
        if !input.password.is_empty() {
            items[index].password = input.password.clone();
        }
        items[index].allow_invalid_certs = input.allow_invalid_certs;
        items[index].id.clone()
    } else {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default();
        let mut sequence = 0u64;
        let id = loop {
            let candidate = format!("{stamp:x}-{sequence:x}");
            if !items.iter().any(|item| item.id == candidate) {
                break candidate;
            }
            sequence += 1;
        };
        items.push(StoredConnection {
            id: id.clone(),
            name: name.into(),
            kind: kind.into(),
            url: input.url.trim().into(),
            host: input.host.trim().into(),
            share: input.share.trim().into(),
            base: input.base.trim().into(),
            username: input.username.trim().into(),
            password: input.password,
            allow_invalid_certs: input.allow_invalid_certs,
            port: input.port,
        });
        id
    };
    let conn = items.iter().find(|c| c.id == id).ok_or("连接不存在")?;
    let plugin_id = format!("org.fiex.{}", conn.kind);
    if !crate::plugin_host::permission_allowed(&plugin_id, "network")? {
        return Err("连接插件尚未安装、启用或未获得网络授权".into());
    }
    crate::plugin_host::global().invoke(
        &plugin_id,
        "connection.validate",
        serde_json::json!({"connection": provider_config(conn)}),
    )?;
    if !conn.password.is_empty() {
        crate::plugin_storage::secret_set(
            &format!("org.fiex.{kind}"),
            &format!("connection-{id}"),
            "password",
            &conn.password,
        )?;
    }
    fs::create_dir_all(mount_path(conn)).map_err(|e| e.to_string())?;
    save_stored(&items)?;
    Ok(public(conn))
}

pub fn remove_connection(id: &str) -> Result<(), String> {
    let mut items = load_stored();
    items.retain(|c| c.id != id);
    save_stored(&items)
}

pub fn is_remote(path: &str) -> bool {
    connection_for_path(path).is_some()
}

/// Canonicalize the nearest existing ancestor, then append validated missing
/// components. Remote entries are virtual and must not require placeholder
/// files simply to resolve their mount path.
fn virtual_target(path: &str) -> Option<PathBuf> {
    let mut ancestor = PathBuf::from(path);
    if !ancestor.is_absolute() {
        ancestor = std::env::current_dir().ok()?.join(ancestor);
    }
    let mut missing = Vec::new();
    while !ancestor.exists() {
        let component = ancestor.file_name()?.to_os_string();
        if component == "." || component == ".." {
            return None;
        }
        missing.push(component);
        if !ancestor.pop() {
            return None;
        }
    }
    let mut target = dunce::canonicalize(ancestor).ok()?;
    for part in missing.into_iter().rev() {
        target.push(part);
    }
    Some(target)
}

fn connection_for_path(path: &str) -> Option<(StoredConnection, PathBuf)> {
    let target = virtual_target(path)?;
    for conn in load_stored() {
        let Ok(root) = dunce::canonicalize(mount_path(&conn)) else {
            continue;
        };
        if target == root || target.starts_with(&root) {
            let rel = target.strip_prefix(&root).ok()?.to_path_buf();
            return Some((conn, rel));
        }
    }
    None
}

fn rel_string(rel: &Path) -> String {
    rel.components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join("/")
}

fn remote_join(base: &str, rel: &str) -> String {
    let mut out = base.trim_matches('/').to_string();
    if !rel.is_empty() {
        if !out.is_empty() {
            out.push('/');
        }
        out.push_str(rel);
    }
    out
}

fn safe_child_name(name: &str) -> Option<&str> {
    let name = name.trim();
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\\') {
        return None;
    }
    Some(name)
}

fn local_for(conn: &StoredConnection, rel: &str) -> PathBuf {
    let mut path = mount_path(conn);
    for part in rel.split('/').filter(|part| !part.is_empty()) {
        path.push(part);
    }
    path
}

async fn list_remote(conn: &StoredConnection, rel: &str) -> Result<Vec<RemoteFile>, String> {
    let plugin_id = format!("org.fiex.{}", conn.kind);
    if !crate::plugin_host::permission_allowed(&plugin_id, "network")? {
        return Err("remote plugin network permission not granted".into());
    }
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
                path: entry
                    .get("nodeId")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(name)
                    .into(),
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
        if (!hidden && file.name.starts_with('.'))
            || filters.is_ignored(Path::new(&file.name))
        {
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

async fn fetch_remote(conn: &StoredConnection, rel: &str, dest: &Path) -> Result<u64, String> {
    // Legacy download transport cannot bypass a revoked plugin authorization.
    // It will be removed when the SDK binary stream transport is in place.
    let plugin_id = format!("org.fiex.{}", conn.kind);
    if !crate::plugin_host::permission_allowed(&plugin_id, "network")? {
        return Err("remote plugin network permission not granted".into());
    }
    if conn.kind == "webdav" {
        webdav_get(conn, rel, dest).await
    } else {
        smb_get(conn, rel, dest).await
    }
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
    let plugin_id = format!("org.fiex.{}", conn.kind);
    if !crate::plugin_host::permission_allowed(&plugin_id, "network")? {
        return Err("remote plugin network permission not granted".into());
    }
    let config = provider_config(conn);
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

pub async fn download(path: &str) -> Result<String, String> {
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
    let size = fetch_remote(&conn, &rel, &temp).await?;
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

pub async fn download_dir(path: &str) -> Result<(), String> {
    let Some((conn, rel)) = connection_for_path(path) else {
        return Ok(());
    };
    let rel = rel_string(&rel);
    let mut meta = load_meta(&conn);
    let mut pending = vec![rel];
    while let Some(current) = pending.pop() {
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
                    let size = fetch_remote(&conn, &child, &temp).await?;
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

pub async fn ensure_downloaded(path: &str) -> Result<String, String> {
    if connection_for_path(path).is_some() {
        download(path).await
    } else {
        Ok(path.into())
    }
}

fn webdav_client(conn: &StoredConnection) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(conn.allow_invalid_certs)
        .timeout(Duration::from_secs(90))
        .build()
        .map_err(|e| e.to_string())
}

fn webdav_url(conn: &StoredConnection, rel: &str) -> String {
    let base = conn.url.trim_end_matches('/');
    if rel.is_empty() {
        format!("{base}/")
    } else {
        let path = rel
            .split('/')
            .filter(|part| !part.is_empty())
            .map(|part| urlencoding::encode(part).into_owned())
            .collect::<Vec<_>>()
            .join("/");
        format!("{base}/{path}")
    }
}

fn auth(req: reqwest::RequestBuilder, conn: &StoredConnection) -> reqwest::RequestBuilder {
    if conn.username.is_empty() {
        req
    } else {
        req.basic_auth(&conn.username, Some(&conn.password))
    }
}

async fn webdav_get(conn: &StoredConnection, rel: &str, dest: &Path) -> Result<u64, String> {
    let client = webdav_client(conn)?;
    let request = client.get(webdav_url(conn, rel));
    let response = auth(request, conn)
        .send()
        .await
        .map_err(|e| format!("WebDAV 下载失败：{e}"))?;
    let response = response.error_for_status().map_err(|e| e.to_string())?;
    let mut file = fs::File::create(dest).map_err(|e| e.to_string())?;
    let mut stream = response.bytes_stream();
    let mut size = 0u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        size += chunk.len() as u64;
    }
    Ok(size)
}

async fn smb_client(conn: &StoredConnection) -> Result<(smb::Client, smb::UncPath), String> {
    let mut config = smb::ClientConfig::default();
    if let Some(port) = conn.port {
        config.connection.port = Some(port);
    }
    let client = smb::Client::new(config);
    let unc = smb::UncPath::from_str(&format!("\\\\{}\\{}", conn.host, conn.share))
        .map_err(|e| e.to_string())?;
    client
        .share_connect(&unc, &conn.username, conn.password.clone())
        .await
        .map_err(|e| format!("SMB 连接失败：{e}"))?;
    Ok((client, unc))
}

fn smb_path(conn: &StoredConnection, rel: &str) -> String {
    let joined = remote_join(&conn.base, rel).replace('/', "\\");
    joined.trim_matches('\\').to_string()
}

async fn smb_get(conn: &StoredConnection, rel: &str, dest: &Path) -> Result<u64, String> {
    let (client, unc) = smb_client(conn).await?;
    let path = smb_path(conn, rel);
    let file = client
        .create_file(
            &unc.with_path(&path),
            &smb::FileCreateArgs::make_open_existing(
                smb::FileAccessMask::new().with_generic_read(true),
            ),
        )
        .await
        .map_err(|e| e.to_string())?
        .unwrap_file();
    let mut output = fs::File::create(dest).map_err(|e| e.to_string())?;
    let mut offset = 0u64;
    let mut buffer = vec![0u8; 1024 * 1024];
    let mut total = 0u64;
    loop {
        let read = file
            .read_at(&mut buffer, offset)
            .await
            .map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        output
            .write_all(&buffer[..read])
            .map_err(|e| e.to_string())?;
        offset += read as u64;
        total += read as u64;
    }
    let _ = file.close().await;
    Ok(total)
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
        let status = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            return Ok(());
        }
        return Err("无法弹出该设备".into());
    }
    #[allow(unreachable_code)]
    Err("当前平台不支持弹出设备".into())
}

pub async fn sync_one(path: &str) -> Result<(), String> {
    let Some((conn, rel)) = connection_for_path(path) else {
        return Ok(());
    };
    let rel = rel_string(&rel);
    let meta = load_meta(&conn);
    if meta.files.get(&rel).is_some_and(|file| file.is_dir) {
        download_dir(path).await
    } else {
        download(path).await.map(|_| ())
    }
}

pub async fn sync(paths: &[String]) -> Result<(), String> {
    for path in paths {
        sync_one(path).await?;
    }
    Ok(())
}

pub fn unsync(paths: &[String]) -> Result<(), String> {
    for path in paths {
        let Some((conn, rel)) = connection_for_path(path) else {
            continue;
        };
        let rel = rel_string(&rel);
        let mut meta = load_meta(&conn);
        let local = local_for(&conn, &rel);
        if local.is_dir() {
            let prefix = format!("{rel}/");
            let keys = meta
                .files
                .iter()
                .filter(|(key, _)| key.as_str() == rel || key.starts_with(&prefix))
                .map(|(key, _)| key.clone())
                .collect::<Vec<_>>();
            for key in keys {
                let child = local_for(&conn, &key);
                if meta.files.get(&key).is_some_and(|file| !file.is_dir) {
                    let _ = fs::remove_file(&child);

                }
                if let Some(file) = meta.files.get_mut(&key) {
                    file.downloaded = false;
                }
            }
        } else {
            let _ = fs::remove_file(&local);
            if let Some(file) = meta.files.get_mut(&rel) {
                file.downloaded = false;
            }
        }
        save_meta(&conn, &meta)?;
    }
    Ok(())
}
