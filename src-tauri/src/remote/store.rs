//! Connection registry: where it lives on disk, how it is loaded and stored,
//! and the public projection handed to the frontend.

use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use super::config::{
    connection_domain, declared_connection_fields, ensure_provider_access, mount_path,
    provider_config,
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredConnection {
    pub(crate) id: String,
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
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub plugin_id: String,
    pub mount_path: String,
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
    let mut items: Vec<StoredConnection> = fs::read(connections_path())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    for item in &mut items {
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

fn public(conn: &StoredConnection) -> ConnectionInfo {
    let mount = mount_path(conn);
    let _ = fs::create_dir_all(&mount);
    ConnectionInfo {
        id: conn.id.clone(),
        name: conn.name.clone(),
        kind: conn.kind.clone(),
        plugin_id: connection_plugin_id(conn),
        mount_path: mount.to_string_lossy().into_owned(),
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
    if kind.is_empty() || kind.len() > 80 || kind.contains(['/', '\\', '\0']) {
        return Err("连接类型无效".into());
    }
    let plugin_id = input
        .plugin_id
        .clone()
        .ok_or("connection plugin ID is required")?;
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
    let mut record = StoredConnection {
        id: id.clone(),
        name: name.into(),
        kind: kind.into(),
        plugin_id: plugin_id.clone(),
        settings,
        secret_keys,
    };
    ensure_provider_access(&record)?;
    // Saving a connection is explicit user intent to reach this host. The
    // exact hostname is added to the plugin's own allowlist before validation.
    if let Some(domain) = connection_domain(&record.settings) {
        crate::plugin_host::set_network_domain(&plugin_id, &domain, true)?;
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
    for field in &record.secret_keys {
        record.settings.remove(field);
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
