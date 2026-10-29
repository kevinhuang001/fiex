use super::*;

#[tauri::command]
pub(crate) async fn file_info_details(
    path: String,
) -> Vec<crate::plugins::resolvers::info::InfoSection> {
    use crate::infrastructure::cache::memory::MemoryCache;
    use crate::plugins::resolvers::info::InfoSection;
    use std::sync::OnceLock;
    use std::time::Duration;
    static INFO: OnceLock<MemoryCache<Vec<InfoSection>>> = OnceLock::new();
    tauri::async_runtime::spawn_blocking(move || {
        let cache = INFO.get_or_init(|| MemoryCache::new(256, Duration::from_secs(90)));
        // No metadata read may bypass the currently authorized storage owner.
        let resource = crate::core::fs::trusted(&path, "core:info")?;
        resource.require(crate::plugins::provider_registry::FsOperation::Stat, false)?;
        let metadata = resource.desktop_entry().map_err(String::from)?;
        let key = serde_json::to_string(&(
            &path, metadata.modified, metadata.size, metadata.mime.as_str(),
            crate::i18n::locale(), crate::plugins::preferences::snapshot(),
        )).map_err(|e| e.to_string())?;
        if let Some(value) = cache.get(&key) { return Ok::<_, String>(value); }
        let result = crate::plugins::resolvers::info::file_info_resource(&path)?;
        cache.insert(key, result.clone());
        Ok(result)
    })
    .await
    .ok()
    .and_then(Result::ok)
    .unwrap_or_default()
}

#[tauri::command]
pub(crate) fn plugin_task(
    plugin_id: String,
    task_id: String,
) -> Option<crate::plugins::host::PluginTaskInfo> {
    crate::plugins::host::global()
        .task(&task_id)
        .filter(|task| task.plugin_id == plugin_id)
}

#[tauri::command]
pub(crate) fn plugin_cancel_task(plugin_id: String, task_id: String) -> Result<(), String> {
    let task = crate::plugins::host::global()
        .task(&task_id)
        .filter(|task| task.plugin_id == plugin_id)
        .ok_or("background task not found")?;
    crate::plugins::host::global().cancel_task(&task.task_id)
}

#[tauri::command]
pub(crate) fn plugin_status() -> Vec<(String, bool)> {
    crate::plugins::host::catalog()
        .into_iter()
        .map(|item| (item.id, item.backend_available))
        .collect()
}

#[tauri::command]
pub(crate) async fn plugin_decorations(
    directory: String,
    entries: Vec<crate::plugins::resolvers::decorations::DecorationEntry>,
    include_plugins: Option<bool>,
) -> Result<Vec<crate::plugins::resolvers::decorations::Decoration>, String> {
    if !directory.starts_with('/') && !directory.contains(":\\") && !directory.contains("://") {
        return Err("invalid decoration directory".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let should_cache = include_plugins.unwrap_or(true);
        let values = crate::plugins::resolvers::decorations::query_with_plugins(
            &directory,
            entries.clone(),
            should_cache,
        );
        if should_cache {
            crate::core::fs::directory_cache::store_decorations(&directory, &entries, &values);
        }
        values
    })
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn plugin_themes() -> Vec<crate::plugins::themes::Theme> {
    crate::plugins::host::themes()
}

#[tauri::command]
pub(crate) fn plugin_network_domains(plugin_id: String) -> Result<Vec<String>, String> {
    crate::plugins::host::network_domains(&plugin_id)
}

#[tauri::command]
pub(crate) fn plugin_permission_allowed(
    plugin_id: String,
    permission: String,
) -> Result<bool, String> {
    crate::plugins::host::permission_allowed(&plugin_id, &permission)
}

#[tauri::command]
pub(crate) fn plugin_panel_html(plugin_id: String, panel_id: String) -> Result<String, String> {
    crate::plugins::host::panel_html(&plugin_id, &panel_id).inspect_err(|error| {
        record_plugin_error(&plugin_id, "panel", &format!("{panel_id}: {error}"))
    })
}

#[tauri::command]
pub(crate) fn plugin_view_asset(plugin_id: String, entry: String) -> Result<String, String> {
    crate::plugins::host::plugin_view_asset(&plugin_id, &entry).inspect_err(|error| {
        record_plugin_error(&plugin_id, "preview", &format!("view asset: {error}"))
    })
}

#[tauri::command]
pub(crate) fn plugin_panel_asset(plugin_id: String, panel_id: String) -> Result<String, String> {
    crate::plugins::host::panel_asset(&plugin_id, &panel_id).inspect_err(|error| {
        record_plugin_error(&plugin_id, "panel", &format!("{panel_id}: {error}"))
    })
}
