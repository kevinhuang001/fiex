//! Connection registry: where it lives on disk, how it is loaded and stored,
//! and the public projection handed to the frontend.

use super::config::{
    connection_domain, declared_connection_fields, ensure_provider_access, mount_path,
    provider_config,
};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

mod configuration;
pub use configuration::*;

mod navigation;
pub use navigation::*;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredConnection {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) mount_id: String,
    pub(crate) name: String,
    pub(crate) kind: String,
    /// The exact installed provider ID, never derived from publisher privilege.
    #[serde(default)]
    pub(crate) plugin_id: String,
    /// Non-secret manifest-declared connection configuration.
    #[serde(default)]
    pub(crate) settings: serde_json::Map<String, serde_json::Value>,
    /// Secret field identifiers whose values are persisted in the OS keyring.
    #[serde(default)]
    pub(crate) secret_keys: Vec<String>,
    /// Saved sessions appear in the connection + menu.
    #[serde(default = "saved_default")]
    pub(crate) saved: bool,
    /// Open this session automatically when Fiex starts.
    #[serde(default)]
    pub(crate) autostart: bool,
}

fn saved_default() -> bool {
    true
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInput {
    pub id: Option<String>,
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub plugin_id: Option<String>,
    #[serde(default)]
    pub settings: serde_json::Map<String, serde_json::Value>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub plugin_id: String,
    pub mount_path: String,
    pub saved: bool,
    pub autostart: bool,
    /// Provider can enumerate and read this mount.
    pub can_read: bool,
    /// Provider can create/upload into this mount.
    pub can_write: bool,
}

pub(crate) fn fiex_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".fiex")
}

fn connections_path() -> PathBuf {
    fiex_dir().join("connections.json")
}

pub(crate) fn connection_plugin_id(conn: &StoredConnection) -> String {
    conn.plugin_id.clone()
}

pub(crate) fn load_stored() -> Vec<StoredConnection> {
    load_stored_with_secrets(true)
}

/// Sidebar and picker projections never inspect credentials. Reading the OS
/// keyring for every connection on every sidebar refresh is both slow and
/// unnecessary; only operations that actually open a provider use this flag.
pub(crate) fn load_stored_public() -> Vec<StoredConnection> {
    load_stored_with_secrets(false)
}

fn load_stored_with_secrets(hydrate: bool) -> Vec<StoredConnection> {
    // Sidebar and the background scheduler may both be the first reader of an
    // old registry. Serialize assignment so its new identity is persisted once.
    static MIGRATION: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let Ok(_guard) = MIGRATION.lock() else {
        return Vec::new();
    };
    let mut items: Vec<StoredConnection> = fs::read(connections_path())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let mut changed = false;
    items.retain(|c| {
        !c.id.is_empty()
            && c.id.len() <= 128
            && c.id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    });
    let mut counts = std::collections::HashMap::<String, usize>::new();
    for item in &items {
        *counts.entry(format!("id:{}", item.id)).or_default() += 1;
        if !item.mount_id.is_empty() {
            *counts
                .entry(format!("mount:{}", item.mount_id))
                .or_default() += 1;
        }
    }
    items.retain(|c| {
        counts[&format!("id:{}", c.id)] == 1
            && (c.mount_id.is_empty() || counts[&format!("mount:{}", c.mount_id)] == 1)
    });
    let mut identities = std::collections::HashSet::new();
    for item in &mut items {
        if item.mount_id.is_empty() {
            let Ok(id) = crate::plugins::mounts::new_id() else {
                continue;
            };
            item.mount_id = id;
            changed = true;
        }
        if crate::plugins::mounts::uri(&item.mount_id, "").is_err()
            || !identities.insert(item.mount_id.clone())
        {
            // Never silently redirect a duplicated or corrupted identity.
            item.mount_id.clear();
        }
    }
    items.retain(|c| !c.mount_id.is_empty());
    if changed && save_stored(&items).is_err() {
        return Vec::new();
    }
    if hydrate {
        for item in &mut items {
            for field in &item.secret_keys {
                if let Ok(value) = crate::plugins::storage::secret_get(
                    &connection_plugin_id(item),
                    &format!("connection-{}", item.id),
                    field,
                ) {
                    item.settings.insert(field.clone(), value.into());
                }
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

mod projection;

use projection::public;

pub use projection::{connection_config, list_connections};
