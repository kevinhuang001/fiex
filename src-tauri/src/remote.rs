use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredConnection {
    id: String,
    name: String,
    kind: String,
    /// The exact installed provider ID, never derived from publisher privilege.
    #[serde(default)]
    plugin_id: String,
    /// Non-secret manifest-declared connection configuration.
    #[serde(default)]
    settings: serde_json::Map<String, serde_json::Value>,
    /// Secret field identifiers whose values are persisted in the OS keyring.
    #[serde(default)]
    secret_keys: Vec<String>,
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
    pub plugin_id: Option<String>,
    #[serde(default)]
    pub settings: serde_json::Map<String, serde_json::Value>,
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
    pub plugin_id: String,
    pub mount_path: String,
    pub host: String,
    pub url: String,
    pub share: String,
    pub base: String,
}

#[derive(Clone)]
struct RemoteFile {
    name: String,
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

fn connection_plugin_id(conn: &StoredConnection) -> String {
    if conn.plugin_id.is_empty() {
        // Migrate v0.2.7 connection records that predate provider IDs.
        format!("org.fiex.{}", conn.kind)
    } else {
        conn.plugin_id.clone()
    }
}

/// Refuse requests when a provider has lost one of its declared permissions.
/// Remote providers may only read; Core owns the destination file and cache.
fn ensure_provider_access(conn: &StoredConnection) -> Result<String, String> {
    let plugin_id = connection_plugin_id(conn);
    let plugin = crate::plugin_host::catalog()
        .into_iter()
        .find(|plugin| plugin.id == plugin_id && plugin.enabled && plugin.backend_available)
        .ok_or("连接插件未安装、启用或已被撤销授权")?;
    let mut required = vec!["filesystem.read"];
    if plugin.declared_permissions.iter().any(|p| p == "network") {
        required.push("network");
    }
    for permission in required {
        if !crate::plugin_host::permission_allowed(&plugin_id, permission)? {
            return Err(format!("插件权限未授权：{permission}"));
        }
    }
    Ok(plugin_id)
}

/// Find the user-installed and approved provider that declares this factory.
/// No official or trusted-publisher shortcut is permitted.
fn declared_connection_fields(
    plugin_id: &str,
    kind: &str,
) -> Result<Vec<(String, String)>, String> {
    let plugin = crate::plugin_host::catalog()
        .into_iter()
        .find(|plugin| plugin.id == plugin_id && plugin.enabled && plugin.backend_available)
        .ok_or("连接插件尚未安装、启用或授权")?;
    let factory = plugin
        .contributes
        .get("connectionFactories")
        .and_then(serde_json::Value::as_array)
        .and_then(|factories| {
            factories
                .iter()
                .find(|factory| factory.get("id").and_then(serde_json::Value::as_str) == Some(kind))
        })
        .ok_or("插件未声明请求的连接类型")?;
    let mut fields = Vec::new();
    for field in factory
        .get("fields")
        .and_then(serde_json::Value::as_array)
        .ok_or("连接插件的字段声明无效")?
    {
        let key = field
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or("连接字段缺少 ID")?;
        let ty = field
            .get("type")
            .and_then(serde_json::Value::as_str)
            .ok_or("连接字段缺少类型")?;
        if key.is_empty() || key.len() > 80 || fields.iter().any(|(id, _)| id == key) {
            return Err("连接字段 ID 无效或重复".into());
        }
        if !matches!(ty, "text" | "url" | "number" | "checkbox" | "secret") {
            return Err("连接字段类型不支持".into());
        }
        fields.push((key.to_string(), ty.to_string()));
    }
    Ok(fields)
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
                &connection_plugin_id(item),
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
                &connection_plugin_id(item),
                &format!("connection-{}", item.id),
                "password",
            )
            .unwrap_or_default();
        }
        for field in &item.secret_keys {
            if let Ok(value) = crate::plugin_storage::secret_get(
                &connection_plugin_id(item),
                &format!("connection-{}", item.id),
                field,
            ) {
                item.settings.insert(field.clone(), value.into());
            }
        }
    }
    items
}

fn save_stored(items: &[StoredConnection]) -> Result<(), String> {
    fs::create_dir_all(fiex_dir()).map_err(|e| e.to_string())?;
    let path = connections_path();
    // Fail closed: in-memory credentials may have been rehydrated from the
    // keyring. Strip every declared secret before writing the registry.
    let safe = items
        .iter()
        .cloned()
        .map(|mut item| {
            for key in &item.secret_keys {
                item.settings.remove(key);
            }
            item.password.clear();
            item
        })
        .collect::<Vec<_>>();
    fs::write(
        &path,
        serde_json::to_vec_pretty(&safe).map_err(|e| e.to_string())?,
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
fn allowed_network_domains(plugin_id: &str) -> Vec<String> {
    let Some(plugin) = crate::plugin_host::catalog()
        .into_iter()
        .find(|plugin| plugin.id == plugin_id)
    else {
        return Vec::new();
    };
    let declared = plugin
        .declared_permissions
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let mut domains = crate::plugin_permissions::status(plugin_id, &declared)
        .map(|state| state.network_domains.into_iter().collect::<Vec<_>>())
        .unwrap_or_default();
    for loopback in ["localhost", "127.0.0.1", "::1"] {
        if !domains.iter().any(|domain| domain == loopback) {
            domains.push(loopback.into());
        }
    }
    domains
}

fn provider_config(conn: &StoredConnection) -> serde_json::Value {
    let mut config = serde_json::json!({
        "url": conn.url,
        "host": conn.host,
        "port": conn.port,
        "share": conn.share,
        "base": conn.base,
        "username": conn.username,
        "password": conn.password,
        "allowInvalidCerts": conn.allow_invalid_certs
    });
    if let Some(map) = config.as_object_mut() {
        map.insert(
            "networkDomains".into(),
            serde_json::json!(allowed_network_domains(&connection_plugin_id(conn))),
        );
        map.extend(conn.settings.clone());
    }
    config
}

fn public(conn: &StoredConnection) -> ConnectionInfo {
    let mount = mount_path(conn);
    let _ = fs::create_dir_all(&mount);
    ConnectionInfo {
        id: conn.id.clone(),
        name: conn.name.clone(),
        kind: conn.kind.clone(),
        plugin_id: connection_plugin_id(conn),
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

fn connection_domain(kind: &str, url: &str, host: &str) -> Option<String> {
    let value = match kind {
        "webdav" => reqwest::Url::parse(url)
            .ok()
            .and_then(|url| url.host_str().map(str::to_owned))?,
        "smb" => host.trim().to_owned(),
        _ => return None,
    };
    crate::plugin_permissions::normalize_domain(&value).ok()
}

pub fn save_connection(input: ConnectionInput) -> Result<ConnectionInfo, String> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err("请输入连接名称".into());
    }
    let kind = input.kind.trim();
    if kind.is_empty() || kind.len() > 80 || kind.contains(['/', '\\', '\0']) {
        return Err("连接类型无效".into());
    }
    let plugin_id = input
        .plugin_id
        .clone()
        .unwrap_or_else(|| format!("org.fiex.{kind}"));
    if !crate::plugin_host::valid_id(&plugin_id) {
        return Err("连接插件 ID 无效".into());
    }
    let declared_fields = declared_connection_fields(&plugin_id, kind)?;
    for (key, value) in &input.settings {
        let field_type = declared_fields
            .iter()
            .find(|(id, _)| id == key)
            .map(|(_, ty)| ty.as_str())
            .ok_or_else(|| format!("连接插件未声明字段：{key}"))?;
        let valid = match field_type {
            "checkbox" => value.is_boolean(),
            "number" => value.is_number(),
            _ => value.is_string(),
        };
        if !valid {
            return Err(format!("连接字段数据类型无效：{key}"));
        }
    }
    let secret_keys = declared_fields
        .into_iter()
        .filter(|(_, ty)| ty == "secret")
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    let value_string = |field: &str| {
        input
            .settings
            .get(field)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string()
    };
    let url = if input.url.trim().is_empty() {
        value_string("url")
    } else {
        input.url.trim().to_string()
    };
    let host = if input.host.trim().is_empty() {
        value_string("host")
    } else {
        input.host.trim().to_string()
    };
    let share = if input.share.trim().is_empty() {
        value_string("share")
    } else {
        input.share.trim().to_string()
    };
    if kind == "webdav" && !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("WebDAV 地址必须以 http:// 或 https:// 开头".into());
    }
    if kind == "smb" && (host.is_empty() || share.is_empty()) {
        return Err("请填写 SMB 主机和共享名".into());
    }
    if input.port == Some(0) {
        return Err("请输入有效的端口".into());
    }

    let mut items = load_stored();
    let existing = input
        .id
        .as_deref()
        .and_then(|id| items.iter().position(|conn| conn.id == id));
    let previous = existing.map(|index| items[index].clone());
    if previous
        .as_ref()
        .is_some_and(|conn| connection_plugin_id(conn) != plugin_id || conn.kind != kind)
    {
        return Err("更新连接时不得改变所属插件和连接类型".into());
    }
    let id = match previous.as_ref() {
        Some(conn) => conn.id.clone(),
        None => {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_millis())
                .unwrap_or_default();
            let mut sequence = 0u64;
            loop {
                let candidate = format!("{stamp:x}-{sequence:x}");
                if !items.iter().any(|item| item.id == candidate) {
                    break candidate;
                }
                sequence += 1;
            }
        }
    };
    let mut settings = input.settings;
    // An empty value in an update keeps the existing keyring credential;
    // only declared "secret" fields are ever persisted in that keyring.
    for key in &secret_keys {
        let empty = settings
            .get(key)
            .and_then(serde_json::Value::as_str)
            .is_none_or(str::is_empty);
        if empty {
            if let Some(value) = previous.as_ref().and_then(|conn| conn.settings.get(key)) {
                settings.insert(key.clone(), value.clone());
            }
        }
    }
    let password = settings
        .get("password")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .or_else(|| (!input.password.is_empty()).then_some(input.password.clone()))
        .or_else(|| previous.as_ref().map(|conn| conn.password.clone()))
        .unwrap_or_default();
    let record = StoredConnection {
        id: id.clone(),
        name: name.into(),
        kind: kind.into(),
        plugin_id: plugin_id.clone(),
        settings,
        secret_keys,
        url,
        host,
        share,
        base: input.base.trim().into(),
        username: input.username.trim().into(),
        password,
        allow_invalid_certs: input.allow_invalid_certs,
        port: input.port,
    };
    ensure_provider_access(&record)?;
    // Saving a connection is explicit user intent to reach this host. The
    // exact hostname is added to the plugin's own allowlist before validation.
    if let Some(domain) = connection_domain(&record.kind, &record.url, &record.host) {
        if !crate::plugin_permissions::is_loopback_domain(&domain) {
            crate::plugin_host::set_network_domain(&plugin_id, &domain, true)?;
        }
    }
    crate::plugin_host::global().invoke(
        &plugin_id,
        "connection.validate",
        serde_json::json!({"connection": provider_config(&record)}),
    )?;
    let scope = format!("connection-{id}");
    for field in &record.secret_keys {
        if let Some(value) = record
            .settings
            .get(field)
            .and_then(serde_json::Value::as_str)
        {
            if !value.is_empty() {
                crate::plugin_storage::secret_set(&plugin_id, &scope, field, value)?;
            }
        }
    }
    // Old records may still supply their password through legacy UI arguments.
    if !record.password.is_empty() && !record.secret_keys.iter().any(|key| key == "password") {
        crate::plugin_storage::secret_set(&plugin_id, &scope, "password", &record.password)?;
    }
    fs::create_dir_all(mount_path(&record)).map_err(|e| e.to_string())?;
    if let Some(index) = existing {
        items[index] = record.clone();
    } else {
        items.push(record.clone());
    }
    save_stored(&items)?;
    Ok(public(&record))
}

pub fn remove_connection(id: &str) -> Result<(), String> {
    let mut items = load_stored();
    let removed = items.iter().find(|conn| conn.id == id).cloned();
    items.retain(|conn| conn.id != id);
    save_stored(&items)?;
    if let Some(conn) = removed {
        let scope = format!("connection-{id}");
        let plugin_id = connection_plugin_id(&conn);
        for field in &conn.secret_keys {
            let _ = crate::plugin_storage::secret_delete(&plugin_id, &scope, field);
        }
        let _ = crate::plugin_storage::secret_delete(&plugin_id, &scope, "password");
    }
    Ok(())
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

pub async fn sync_one(path: &str, options: &TransferOptions) -> Result<(), String> {
    let Some((conn, rel)) = connection_for_path(path) else {
        return Ok(());
    };
    let rel = rel_string(&rel);
    let meta = load_meta(&conn);
    if meta.files.get(&rel).is_some_and(|file| file.is_dir) {
        download_dir(path, options).await
    } else {
        download(path, options).await.map(|_| ())
    }
}

pub async fn sync(paths: &[String], options: &TransferOptions) -> Result<(), String> {
    for path in paths {
        sync_one(path, options).await?;
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
