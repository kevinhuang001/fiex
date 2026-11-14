//! Provider configuration and permissions, plus remote mount-path resolution.

use std::path::{Path, PathBuf};

use super::store::{StoredConnection, connection_plugin_id, fiex_dir, load_stored};

mod permissions;
pub(crate) use permissions::{declared_connection_fields, ensure_provider_access};

fn drives_dir() -> PathBuf {
    fiex_dir().join("drives")
}

pub(crate) fn mount_path(conn: &StoredConnection) -> PathBuf {
    drives_dir().join(&conn.id)
}

pub(crate) fn provider_config(conn: &StoredConnection) -> serde_json::Value {
    let mut config = conn.settings.clone();
    let scope = format!("connection-{}", conn.id);
    for field in &conn.secret_keys {
        // A newly entered secret takes precedence over the previously saved value.
        if config
            .get(field)
            .and_then(serde_json::Value::as_str)
            .is_some_and(|v| !v.is_empty())
        {
            continue;
        }
        if let Ok(value) =
            crate::plugins::storage::secret_get(&connection_plugin_id(conn), &scope, field)
            && !value.is_empty()
        {
            config.insert(field.clone(), serde_json::Value::String(value));
        }
    }
    // This is a Core-owned policy projection, never user-editable connection
    // data. Apply it last so stale or provider-declared settings cannot widen
    // the effective network allowlist.
    config.insert(
        "networkDomains".into(),
        serde_json::json!(permissions::allowed_network_domains(&connection_plugin_id(
            conn
        ))),
    );
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
        if let Ok(domain) = crate::plugins::security::permissions::normalize_domain(&host) {
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
    crate::plugins::mounts::uri(&conn.mount_id, relative)
}

pub fn connection_for_path(path: &str) -> Option<(StoredConnection, PathBuf)> {
    if crate::plugins::mounts::handles(path) {
        let (uri, source) = crate::plugins::mounts::resolve(path).ok()?;
        return match source {
            crate::plugins::mounts::Source::Connection(conn) => {
                Some((conn, PathBuf::from(uri.relative)))
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use fiex_provider_contracts::ProviderInput;
    use serde_json::json;

    #[test]
    fn effective_network_allowlist_stays_inside_typed_connection_config() {
        let connection = StoredConnection {
            id: "connection-fixture".into(),
            mount_id: "mount-fixture".into(),
            cache_revision: "revision-fixture".into(),
            name: "fixture".into(),
            kind: "fixture".into(),
            plugin_id: "missing.fixture".into(),
            settings: json!({"networkDomains": ["untrusted.example"]})
                .as_object()
                .unwrap()
                .clone(),
            secret_keys: Vec::new(),
            saved: true,
            autostart: false,
        };
        let params = json!({
            "connection": provider_config(&connection),
            "connectionId": connection.id,
        });

        let decoded = ProviderInput::decode("connection.validate", &params).unwrap();
        let ProviderInput::Connection(request) = decoded else {
            panic!("connection.validate must decode to ConnectionRequest");
        };
        let config = request.connection.unwrap().to_value().unwrap();
        assert_eq!(config["networkDomains"], json!([]));
        assert!(
            ProviderInput::decode("connection.validate", &json!({"networkDomains": []})).is_err()
        );
    }
}
