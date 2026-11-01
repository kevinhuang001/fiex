use super::*;

/// A connection without a filesystem contribution opens its plugin-owned view.
pub(super) fn plugin_view_connection(conn: &StoredConnection) -> bool {
    !crate::plugins::host::catalog()
        .into_iter()
        .find(|plugin| plugin.id == conn.plugin_id)
        .is_some_and(|plugin| {
            plugin
                .extensions
                .get("vfs")
                .is_some_and(serde_json::Value::is_object)
        })
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
    if !crate::plugins::host::valid_id(&plugin_id) {
        return Err(crate::i18n::t("连接插件 ID 无效").into());
    }
    let declared_fields = declared_connection_fields(&plugin_id, kind)?;
    for (key, value) in &input.settings {
        let field_type = declared_fields
            .iter()
            .find(|(id, _)| id == key)
            .map(|(_, ty)| ty.as_str())
            .ok_or_else(|| {
                crate::i18n::format("连接插件未声明字段：{key}", &[("key", key.to_string())])
            })?;
        let valid = match field_type {
            "checkbox" => value.is_boolean(),
            "number" => value.is_number(),
            _ => value.is_string(),
        };
        if !valid {
            return Err(crate::i18n::format(
                "连接字段数据类型无效：{key}",
                &[("key", key.to_string())],
            ));
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
    let cache_revision = crate::plugins::mounts::new_id()?;
    let mut record = StoredConnection {
        id: id.clone(),
        mount_id: previous
            .as_ref()
            .map(|c| c.mount_id.clone())
            .unwrap_or(crate::plugins::mounts::new_id()?),
        cache_revision: previous
            .as_ref()
            .map(|c| c.cache_revision.clone())
            .unwrap_or(cache_revision),
        name: name.into(),
        kind: kind.into(),
        plugin_id: plugin_id.clone(),
        settings,
        secret_keys,
        saved: previous.as_ref().map(|conn| conn.saved).unwrap_or(true),
        autostart: previous.as_ref().is_some_and(|conn| conn.autostart),
    };
    {
        ensure_provider_access(&record)?;
        // Saving a connection is explicit user intent to reach this host. The
        // exact hostname is added to the plugin's own allowlist before validation.
        if let Some(domain) = connection_domain(&record.settings) {
            crate::plugins::host::set_network_domain(&plugin_id, &domain, true)?;
        }
        let validation = crate::plugins::resolvers::connection::invoke(
            &plugin_id,
            &record.kind,
            &record.mount_id,
            "connection.validate",
            serde_json::json!({"connection": provider_config(&record), "connectionId":id}),
        )?;
        super::super::credentials::apply_validation(
            &mut record.settings,
            &record.secret_keys,
            validation,
        )?;
    }
    // A connection edit can replace the actual account while leaving its URI
    // and non-secret settings unchanged. Compare the fully hydrated validated
    // configuration here, before removing the private fields.
    let identity_changed = previous.as_ref().is_some_and(|old| {
        old.settings != record.settings || old.secret_keys != record.secret_keys
    });
    if identity_changed || record.cache_revision.is_empty() {
        record.cache_revision = crate::plugins::mounts::new_id()?;
    }
    // Fail before touching the keyring if old account data cannot be cleared.
    // On success the subsequent secret update and registry write cannot
    // resurrect the old account's directory/download snapshots.
    if identity_changed {
        if let Some(old) = previous.as_ref() {
            // Revoke every visited directory before replacing the public
            // connection registry. Old snapshots may belong to another account.
            let root = crate::plugins::mounts::uri(&old.mount_id, "")?;
            crate::core::fs::directory_cache::drop_subtree(&root);
            let download_cache = mount_path(old);
            if download_cache.exists() {
                fs::remove_dir_all(&download_cache).map_err(|e| e.to_string())?;
            }
            super::super::sync::forget_meta(old);
        }
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
    if !plugin_view_connection(&record) {
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
    if let Some(conn) = removed.as_ref() {
        let root = crate::plugins::mounts::uri(&conn.mount_id, "")?;
        crate::core::fs::directory_cache::drop_subtree(&root);
    }
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
        super::super::sync::forget_meta(&conn);
    }
    Ok(())
}
