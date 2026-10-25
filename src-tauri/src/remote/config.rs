//! Provider configuration and permissions, plus remote mount-path resolution.

use std::path::{Path, PathBuf};

use super::store::{connection_plugin_id, fiex_dir, load_stored, StoredConnection};

fn drives_dir() -> PathBuf {
    fiex_dir().join("drives")
}

pub(crate) fn mount_path(conn: &StoredConnection) -> PathBuf {
    drives_dir().join(&conn.id)
}

/// Refuse requests when a provider has lost one of its declared permissions.
/// Core owns caches and upload snapshots; providers receive scoped read access.
pub(crate) fn ensure_provider_access(conn: &StoredConnection) -> Result<String, String> {
    let plugin_id = connection_plugin_id(conn);
    if plugin_id == BUILTIN_PLUGIN_ID {
        if conn.kind == BUILTIN_SSH {
            return Ok(plugin_id);
        }
        return Err("内置连接类型不支持".into());
    }
    let plugin = crate::plugin_host::catalog()
        .into_iter()
        .find(|plugin| {
            plugin.id == plugin_id && plugin.enabled && plugin.approved && plugin.backend_available
        })
        .ok_or("连接插件未安装、启用或已被撤销授权")?;
    declared_connection_fields(&plugin_id, &conn.kind)?;
    let declared = plugin.declared_permissions.iter().cloned().collect();
    crate::plugin_permission_policy::PermissionPolicy::new(&plugin_id, &declared)
        .require_all()
        .map_err(|_| "插件权限未授权".to_string())?;
    Ok(plugin_id)
}

pub(crate) const BUILTIN_PLUGIN_ID: &str = "__builtin__";
pub(crate) const BUILTIN_SSH: &str = "ssh";

pub(crate) fn builtin_connection_fields(kind: &str) -> Option<Vec<(String, String)>> {
    if kind != BUILTIN_SSH {
        return None;
    }
    Some(
        [
            ("host", "text"),
            ("port", "number"),
            ("username", "text"),
            ("identityFile", "text"),
            ("terminalType", "text"),
        ]
        .into_iter()
        .map(|(id, ty)| (id.to_string(), ty.to_string()))
        .collect(),
    )
}

/// Find the user-installed and approved provider that declares this factory.
/// No official or trusted-publisher shortcut is permitted.
pub(crate) fn declared_connection_fields(
    plugin_id: &str,
    kind: &str,
) -> Result<Vec<(String, String)>, String> {
    if plugin_id == BUILTIN_PLUGIN_ID {
        return builtin_connection_fields(kind).ok_or_else(|| "内置连接类型不支持".to_string());
    }
    let plugin = crate::plugin_host::catalog()
        .into_iter()
        .find(|plugin| {
            plugin.id == plugin_id && plugin.enabled && plugin.approved && plugin.backend_available
        })
        .ok_or("连接插件尚未安装、启用或授权")?;
    let factory = plugin
        .extensions
        .get("connections")
        .filter(|factory| factory.is_object())
        .ok_or("插件未声明连接类型")?;
    let _ = kind;
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

pub(crate) fn provider_config(conn: &StoredConnection) -> serde_json::Value {
    let mut config = serde_json::Map::new();
    config.insert(
        "networkDomains".into(),
        serde_json::json!(allowed_network_domains(&connection_plugin_id(conn))),
    );
    config.extend(conn.settings.clone());
    let scope = format!("connection-{}", conn.id);
    for field in &conn.secret_keys {
        if let Ok(value) =
            crate::plugin_storage::secret_get(&connection_plugin_id(conn), &scope, field)
        {
            if !value.is_empty() {
                config.insert(field.clone(), serde_json::Value::String(value));
            }
        }
    }
    serde_json::Value::Object(config)
}

pub(crate) fn connection_domain(
    settings: &serde_json::Map<String, serde_json::Value>,
) -> Option<String> {
    for key in ["url", "host", "server", "address"] {
        let Some(value) = settings.get(key).and_then(serde_json::Value::as_str) else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        let host = value
            .parse::<reqwest::Url>()
            .ok()
            .and_then(|url| url.host_str().map(str::to_owned))
            .unwrap_or_else(|| value.split('/').next().unwrap_or(value).to_owned());
        if let Ok(domain) = crate::plugin_permissions::normalize_domain(&host) {
            return Some(domain);
        }
    }
    None
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

pub fn navigation_path(conn: &StoredConnection, relative: &str) -> Result<String, String> {
    crate::mounts::uri(&conn.mount_id, relative)
}

pub fn connection_for_path(path: &str) -> Option<(StoredConnection, PathBuf)> {
    if crate::mounts::handles(path) {
        let (uri, source) = crate::mounts::resolve(path).ok()?;
        return match source {
            crate::mounts::Source::Connection(conn) => Some((conn, PathBuf::from(uri.relative))),
            _ => None,
        };
    }
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

pub(crate) fn rel_string(rel: &Path) -> String {
    rel.components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join("/")
}

pub(crate) fn safe_child_name(name: &str) -> Option<&str> {
    let name = name.trim();
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\\') {
        return None;
    }
    Some(name)
}

pub(crate) fn local_for(conn: &StoredConnection, rel: &str) -> PathBuf {
    let mut path = mount_path(conn);
    for part in rel.split('/').filter(|part| !part.is_empty()) {
        path.push(part);
    }
    path
}

/// Background work must never follow a locally inserted symlink out of a mount.
pub(crate) fn check_cache_path(conn: &StoredConnection, path: &Path) -> Result<(), String> {
    let root = dunce::canonicalize(mount_path(conn)).map_err(|e| e.to_string())?;
    let relative = path
        .strip_prefix(mount_path(conn))
        .map_err(|_| "invalid cache path")?;
    let mut current = root.clone();
    for component in relative.components() {
        if !matches!(component, std::path::Component::Normal(_)) {
            return Err("invalid cache component".into());
        }
        current.push(component);
        if let Ok(metadata) = std::fs::symlink_metadata(&current) {
            if metadata.file_type().is_symlink() {
                return Err("cache path contains a symlink".into());
            }
            let resolved = dunce::canonicalize(&current).map_err(|e| e.to_string())?;
            if !resolved.starts_with(&root) {
                return Err("cache path escaped mount".into());
            }
        }
    }
    Ok(())
}

pub(crate) fn connection_by_mount(id: &str) -> Option<StoredConnection> {
    load_stored().into_iter().find(|c| c.mount_id == id)
}
