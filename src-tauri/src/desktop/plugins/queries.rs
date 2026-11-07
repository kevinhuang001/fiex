use super::*;

#[tauri::command]
pub(crate) async fn file_info_session_open(path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::plugins::providers::info::open_session(path)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn file_info_describe(
    session_id: String,
    on_partial: tauri::ipc::Channel<Vec<crate::plugins::providers::info::InfoSectionView>>,
) -> Result<Vec<crate::plugins::providers::info::InfoSectionView>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::plugins::providers::info::describe_session(&session_id, |sections| {
            let _ = on_partial.send(sections);
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn file_info_compute(
    session_id: String,
    provider_id: String,
    fields: Vec<crate::plugins::contracts::InfoFieldKey>,
    task_id: String,
    user_initiated: bool,
) -> Result<Vec<crate::plugins::providers::info::InfoSectionView>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::plugins::providers::info::compute_session(
            session_id,
            provider_id,
            fields,
            task_id,
            user_initiated,
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) fn file_info_cancel(session_id: String, task_id: String) {
    crate::plugins::providers::info::cancel_session_task(&session_id, &task_id);
}

#[tauri::command]
pub(crate) fn file_info_session_close(session_id: String) {
    crate::plugins::providers::info::close_session(&session_id);
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
    entries: Vec<crate::plugins::providers::decorations::DecorationEntry>,
    include_plugins: Option<bool>,
) -> Result<Vec<crate::plugins::providers::decorations::Decoration>, String> {
    if !directory.starts_with('/') && !directory.contains(":\\") && !directory.contains("://") {
        return Err("invalid decoration directory".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        crate::plugins::providers::decorations::query_with_plugins(
            &directory,
            entries.clone(),
            include_plugins.unwrap_or(true),
        )
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
