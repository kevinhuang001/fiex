use super::super::store::{StoredConnection, connection_plugin_id};

/// Refuse requests when a provider has lost one of its declared permissions.
/// Core owns caches and upload snapshots; providers receive scoped read access.
pub(crate) fn ensure_provider_access(conn: &StoredConnection) -> Result<String, String> {
    let plugin_id = connection_plugin_id(conn);
    let plugin = crate::plugins::host::catalog()
        .into_iter()
        .find(|plugin| {
            plugin.id == plugin_id && plugin.enabled && plugin.approved && plugin.backend_available
        })
        .ok_or(crate::i18n::t("连接插件未安装、启用或已被撤销授权"))?;
    declared_connection_fields(&plugin_id, &conn.kind)?;
    let declared = plugin.declared_permissions.iter().cloned().collect();
    crate::plugins::security::policy::PermissionPolicy::new(&plugin_id, &declared)
        .require_all()
        .map_err(|_| crate::i18n::t("插件权限未授权").to_string())?;
    Ok(plugin_id)
}

/// Find the user-installed and approved provider that declares this factory.
/// No official or trusted-publisher shortcut is permitted.
pub(crate) fn declared_connection_fields(
    plugin_id: &str,
    kind: &str,
) -> Result<Vec<(String, String)>, String> {
    let plugin = crate::plugins::host::catalog()
        .into_iter()
        .find(|plugin| {
            plugin.id == plugin_id && plugin.enabled && plugin.approved && plugin.backend_available
        })
        .ok_or(crate::i18n::t("连接插件尚未安装、启用或授权"))?;
    let factory = plugin
        .extensions
        .get("connections")
        .filter(|factory| factory.is_object())
        .ok_or(crate::i18n::t("插件未声明连接类型"))?;
    let _ = kind;
    let mut fields = Vec::new();
    for field in factory
        .get("fields")
        .and_then(serde_json::Value::as_array)
        .ok_or(crate::i18n::t("连接插件的字段声明无效"))?
    {
        let key = field
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or(crate::i18n::t("连接字段缺少 ID"))?;
        let ty = field
            .get("type")
            .and_then(serde_json::Value::as_str)
            .ok_or(crate::i18n::t("连接字段缺少类型"))?;
        if key.is_empty() || key.len() > 80 || fields.iter().any(|(id, _)| id == key) {
            return Err(crate::i18n::t("连接字段 ID 无效或重复").into());
        }
        if !matches!(
            ty,
            "text" | "url" | "number" | "checkbox" | "secret" | "list" | "file"
        ) {
            return Err(crate::i18n::t("连接字段类型不支持").into());
        }
        fields.push((key.to_string(), ty.to_string()));
    }
    super::super::credentials::append_private_fields(factory, &mut fields)?;
    Ok(fields)
}

/// Explicitly pass credentials only to the selected, authorized provider
/// subprocess; the connection registry itself never serializes passwords.
pub(super) fn allowed_network_domains(plugin_id: &str) -> Vec<String> {
    let Some(plugin) = crate::plugins::host::catalog()
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
    let mut domains = crate::plugins::security::permissions::status(plugin_id, &declared)
        .map(|state| state.network_domains.into_iter().collect::<Vec<_>>())
        .unwrap_or_default();
    for loopback in ["localhost", "127.0.0.1", "::1"] {
        if !domains.iter().any(|domain| domain == loopback) {
            domains.push(loopback.into());
        }
    }
    domains
}
