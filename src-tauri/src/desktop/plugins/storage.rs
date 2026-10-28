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
