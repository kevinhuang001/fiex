//! Connection registry: where it lives on disk, how it is loaded and stored,
//! and the public projection handed to the frontend.

use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

use super::config::{
    connection_domain, declared_connection_fields, ensure_provider_access, mount_path,
    provider_config,
};

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

fn terminal_connection(conn: &StoredConnection) -> bool {
    let plugin = connection_plugin_id(conn);
    (conn.kind == "ssh" && plugin == super::config::BUILTIN_PLUGIN_ID) || plugin == "org.fiex.ssh"
}

pub fn save_connection(input: ConnectionInput) -> Result<ConnectionInfo, String> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(crate::i18n::t("请输入连接名称").into());
    }
    let kind = input.kind.trim();
    if kind.is_empty() || kind.len() > 80 || kind.contains(['/', '\\', '\0']) {
        return Err(crate::i18n::t("连接类型无效").into());
    }
    let plugin_id = input
        .plugin_id
        .clone()
        .ok_or("connection plugin ID is required")?;
    let builtin = plugin_id == super::config::BUILTIN_PLUGIN_ID;
    if !builtin && !crate::plugins::host::valid_id(&plugin_id) {
        return Err(crate::i18n::t("连接插件 ID 无效").into());
    }
    if builtin && kind != super::config::BUILTIN_SSH {
        return Err(crate::i18n::t("内置连接类型不支持").into());
    }
    let declared_fields = declared_connection_fields(&plugin_id, kind)?;
    for (key, value) in &input.settings {
        let field_type = declared_fields
            .iter()
            .find(|(id, _)| id == key)
            .map(|(_, ty)| ty.as_str())
            .ok_or_else(|| {
                crate::i18n::format("连接插件未声明字段：{key}", &[("key", (key).to_string())])
            })?;
        let valid = match field_type {
            "checkbox" => value.is_boolean(),
            "number" => value.is_number(),
            _ => value.is_string(),
        };
        if !valid {
            return Err(crate::i18n::format(
                "连接字段数据类型无效：{key}",
                &[("key", (key).to_string())],
            ));
        }
    }
    if builtin {
        let host = input
            .settings
            .get("host")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or(crate::i18n::t("请输入 SSH 主机"))?;
        if host.starts_with('-')
            || host
                .chars()
                .any(|ch| ch.is_whitespace() || ch == '\0' || ch == '@')
        {
            return Err(crate::i18n::t("SSH 主机格式无效").into());
        }
        if let Some(username) = input
            .settings
            .get("username")
            .and_then(serde_json::Value::as_str)
        {
            if !username.is_empty()
                && (username.starts_with('-')
                    || !username
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-')))
            {
                return Err(crate::i18n::t("SSH 用户名格式无效").into());
            }
        }
        let port = input
            .settings
            .get("port")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(22);
        if !(1..=65535).contains(&port) {
            return Err(crate::i18n::t("SSH 端口无效").into());
        }
        if let Some(path) = input
            .settings
            .get("identityFile")
            .and_then(serde_json::Value::as_str)
        {
            if path.contains('\0') {
                return Err(crate::i18n::t("SSH 私钥路径无效").into());
            }
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
    if input.id.is_some() && existing.is_none() {
        return Err("connection not found".into());
    }
    let previous = existing.map(|index| items[index].clone());
    if previous
        .as_ref()
        .is_some_and(|conn| connection_plugin_id(conn) != plugin_id || conn.kind != kind)
    {
        return Err(crate::i18n::t("更新连接时不得改变所属插件和连接类型").into());
    }
    let id = match previous.as_ref() {
        Some(conn) => conn.id.clone(),
        None => crate::plugins::mounts::new_id()?,
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
        mount_id: previous
            .as_ref()
            .map(|c| c.mount_id.clone())
            .unwrap_or(crate::plugins::mounts::new_id()?),
        name: name.into(),
        kind: kind.into(),
        plugin_id: plugin_id.clone(),
        settings,
        secret_keys,
        saved: previous.as_ref().map(|conn| conn.saved).unwrap_or(true),
        autostart: previous.as_ref().is_some_and(|conn| conn.autostart),
    };
    if !builtin {
        ensure_provider_access(&record)?;
        // Saving a connection is explicit user intent to reach this host. The
        // exact hostname is added to the plugin's own allowlist before validation.
        if let Some(domain) = connection_domain(&record.settings) {
            crate::plugins::host::set_network_domain(&plugin_id, &domain, true)?;
        }
        let validation = crate::plugins::host::global().invoke(
            &plugin_id,
            "connection.validate",
            serde_json::json!({"connection": provider_config(&record), "connectionId":id}),
        )?;
        super::credentials::apply_validation(
            &mut record.settings,
            &record.secret_keys,
            validation,
        )?;
    }
    let scope = format!("connection-{id}");
    for field in &record.secret_keys {
        if let Some(value) = record
            .settings
            .get(field)
            .and_then(serde_json::Value::as_str)
        {
            if !value.is_empty() {
                crate::plugins::storage::secret_set(&plugin_id, &scope, field, value)?;
            }
        }
    }
    for field in &record.secret_keys {
        record.settings.remove(field);
    }
    if !terminal_connection(&record) {
        fs::create_dir_all(mount_path(&record)).map_err(|e| e.to_string())?;
    }
    if let Some(index) = existing {
        items[index] = record.clone();
    } else {
        items.push(record.clone());
    }
    save_stored(&items)?;
    Ok(public(&record))
}

pub fn update_connection_session(
    id: &str,
    saved: Option<bool>,
    autostart: Option<bool>,
) -> Result<ConnectionInfo, String> {
    let mut items = load_stored();
    let connection = items
        .iter_mut()
        .find(|connection| connection.id == id)
        .ok_or("connection not found")?;
    if let Some(value) = saved {
        connection.saved = value;
    }
    if let Some(value) = autostart {
        connection.autostart = value;
    }
    let result = public(connection);
    save_stored(&items)?;
    Ok(result)
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
            let _ = crate::plugins::storage::secret_delete(&plugin_id, &scope, field);
        }
        let _ = crate::plugins::storage::secret_delete(&plugin_id, &scope, "password");
        // Drop every cached byte, pin and sync baseline owned by the
        // connection. A stale cache must never resurface under a new mount.
        let cache = mount_path(&conn);
        if cache.exists() {
            let _ = fs::remove_dir_all(&cache);
        }
        super::sync::forget_meta(&conn);
    }
    Ok(())
}

/// Resolve a saved Connection via its owning plugin. The returned destination
/// can only be a registered VFS mount or vetted HTML from the plugin package.
pub fn open_navigation(id: &str) -> Result<serde_json::Value, String> {
    let connection = load_stored()
        .into_iter()
        .find(|connection| connection.id == id)
        .ok_or("connection not found")?;
    if connection_plugin_id(&connection) == super::config::BUILTIN_PLUGIN_ID {
        if connection.kind == super::config::BUILTIN_SSH {
            return Ok(serde_json::json!({
                "kind": "terminal",
                "connectionId": connection.id,
            }));
        }
        return Err(crate::i18n::t("内置连接类型不支持").into());
    }
    let plugin_id = ensure_provider_access(&connection)?;
    if terminal_connection(&connection) {
        return Ok(serde_json::json!({
            "kind": "terminal",
            "connectionId": connection.id,
        }));
    }
    let target = crate::plugins::events::invoke(
        &plugin_id,
        "connection.open",
        serde_json::json!({
            "connectionId": connection.id,
            "mountId": connection.mount_id,
            "interactionId": crate::plugins::mounts::new_id()?,
            "connection": provider_config(&connection),
        }),
    )?;
    match target.get("kind").and_then(serde_json::Value::as_str) {
        Some("vfs") => {
            let mount_id = target
                .get("mountId")
                .and_then(serde_json::Value::as_str)
                .ok_or("VFS navigation missing mountId")?;
            if mount_id != connection.mount_id {
                return Err("PermissionDenied: foreign connection mount".into());
            }
            let registered = crate::plugins::host::catalog()
                .into_iter()
                .find(|plugin| plugin.id == plugin_id)
                .and_then(|plugin| plugin.extensions.get("vfs").cloned())
                .is_some_and(|vfs| vfs.is_object());
            if !registered {
                return Err("PermissionDenied: VFS extension not registered".into());
            }
            let node_ref = target
                .get("nodeRef")
                .ok_or("VFS navigation missing nodeRef")?;
            if node_ref.get("mountId").and_then(serde_json::Value::as_str)
                != Some(connection.mount_id.as_str())
            {
                return Err("PermissionDenied: foreign nodeRef mount".into());
            }
            let node_id = node_ref
                .get("nodeId")
                .and_then(serde_json::Value::as_str)
                .ok_or("VFS navigation missing nodeId")?;
            Ok(serde_json::json!({
                "kind": "vfs",
                "path": crate::plugins::mounts::uri(&connection.mount_id, node_id)?,
            }))
        }
        Some("pluginView") => {
            let view = target.get("view").ok_or("pluginView missing view")?;
            if view.get("source").and_then(serde_json::Value::as_str) != Some("package") {
                return Err("pluginView source must be its plugin package".into());
            }
            let entry = view
                .get("entry")
                .and_then(serde_json::Value::as_str)
                .ok_or("pluginView missing package entry")?;
            let tab_title = target
                .get("tabTitle")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| {
                    !value.is_empty()
                        && value.chars().count() <= 80
                        && !value.chars().any(char::is_control)
                })
                .map(str::to_owned);
            let state = target
                .get("state")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            if serde_json::to_vec(&state)
                .map_err(|error| error.to_string())?
                .len()
                > 64 * 1024
            {
                return Err("pluginView state exceeds size limit".into());
            }
            let asset_path = crate::plugins::host::plugin_view_asset(&plugin_id, entry)?;
            Ok(serde_json::json!({
                "kind": "pluginView",
                "pluginId": plugin_id,
                "connectionId": id,
                "assetPath": asset_path,
                "state": state,
                "tabTitle": tab_title,
            }))
        }
        _ => Err("Connection must return vfs or pluginView navigation".into()),
    }
}

mod projection;
use projection::public;
pub use projection::{connection_config, list_connections};
