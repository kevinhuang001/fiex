/// Current manifest-declared non-secret settings for one plugin. Panels and
/// PluginViews receive this through `context.get`, so a static UI can react to
/// settings without adding a custom backend RPC. Secrets never leave the
/// keyring through this command.
#[tauri::command]
pub(crate) fn plugin_settings(plugin_id: String) -> Result<serde_json::Value, String> {
    let (manifest, _) = crate::plugins::host::installed_manifests()
        .into_iter()
        .find(|(manifest, _)| manifest.id == plugin_id)
        .ok_or("plugin is not installed")?;
    let Some(fields) = manifest
        .extensions
        .get("settings")
        .and_then(serde_json::Value::as_array)
    else {
        return Ok(serde_json::json!({}));
    };
    let mut values = serde_json::Map::new();
    for field in fields {
        let Some(id) = field.get("id").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if field.get("type").and_then(serde_json::Value::as_str) == Some("secret") {
            continue;
        }
        if let Ok(Some(value)) = crate::plugins::storage::get(&plugin_id, "global", id) {
            values.insert(id.to_string(), value);
        }
    }
    Ok(serde_json::Value::Object(values))
}

#[tauri::command]
pub(crate) fn plugins_list() -> Vec<crate::plugins::host::PluginInfo> {
    crate::plugins::host::catalog()
}

#[tauri::command]
pub(crate) fn plugin_errors(plugin_id: String) -> Vec<crate::plugins::errors::PluginError> {
    crate::plugins::errors::list(&plugin_id)
}

#[tauri::command]
pub(crate) fn plugin_error_report(
    plugin_id: String,
    source: String,
    message: String,
    detail: Option<String>,
) -> Result<(), String> {
    crate::plugins::errors::report(&plugin_id, &source, &message, detail.as_deref())
}

#[tauri::command]
pub(crate) fn plugin_error_clear(plugin_id: String) {
    crate::plugins::errors::clear(&plugin_id);
}

#[tauri::command]
pub(crate) fn plugin_logs(plugin_id: String, level: Option<String>, limit: Option<usize>)
    -> Vec<crate::infrastructure::logging::LogRecord> {
    crate::infrastructure::logging::list(Some(&plugin_id), level.as_deref(), limit.unwrap_or(300))
}
#[tauri::command]
pub(crate) fn core_logs(level: Option<String>, limit: Option<usize>)
    -> Vec<crate::infrastructure::logging::LogRecord> {
    crate::infrastructure::logging::list(None, level.as_deref(), limit.unwrap_or(300))
}
#[tauri::command]
pub(crate) fn plugin_logs_clear(plugin_id: String) {
    crate::infrastructure::logging::clear(Some(&plugin_id));
}
#[tauri::command]
pub(crate) fn state_get(key: String) -> Option<serde_json::Value> {
    crate::infrastructure::state::get(&key)
}
#[tauri::command]
pub(crate) fn state_set(key: String, value: serde_json::Value) -> Result<(), String> {
    crate::infrastructure::state::set(&key, value)
}
#[tauri::command]
pub(crate) fn core_error_report(
    code: String, operation: String, source: String, message: String,
) {
    crate::infrastructure::errors::report(&code, &operation, &source, &message, false);
}
#[tauri::command]
pub(crate) fn error_poll() -> Vec<crate::infrastructure::errors::AppError> {
    crate::infrastructure::errors::drain()
}

#[tauri::command]
pub(crate) fn plugin_storage_get(
    plugin_id: String,
    scope: String,
    key: String,
) -> Result<Option<serde_json::Value>, String> {
    crate::plugins::storage::get(&plugin_id, &scope, &key)
}

#[tauri::command]
pub(crate) fn plugin_storage_set(
    plugin_id: String,
    scope: String,
    key: String,
    value: serde_json::Value,
) -> Result<(), String> {
    crate::plugins::storage::set(&plugin_id, &scope, &key, value)
}

#[tauri::command]
pub(crate) fn plugin_storage_delete(
    plugin_id: String,
    scope: String,
    key: String,
) -> Result<(), String> {
    crate::plugins::storage::delete(&plugin_id, &scope, &key)
}

#[tauri::command]
pub(crate) fn plugin_secret_set(
    plugin_id: String,
    scope: String,
    key: String,
    value: String,
) -> Result<(), String> {
    let permitted = crate::plugins::host::catalog()
        .into_iter()
        .find(|p| p.id == plugin_id)
        .and_then(|p| p.extensions.get("settings").cloned())
        .and_then(|v| v.as_array().cloned())
        .is_some_and(|fields| {
            fields.iter().any(|f| {
                f.get("id").and_then(|v| v.as_str()) == Some(key.as_str())
                    && f.get("type").and_then(|v| v.as_str()) == Some("secret")
            })
        });
    if !permitted || scope != "global" {
        return Err("secret setting is not declared".into());
    }
    crate::plugins::storage::secret_set(&plugin_id, &scope, &key, &value)
}

#[tauri::command]
pub(crate) fn plugin_secret_available() -> bool {
    crate::plugins::storage::secret_available()
}
