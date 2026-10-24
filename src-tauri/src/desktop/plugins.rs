//! Plugin commands: host invocation, status, panels, permissions, storage,
//! marketplace installation, decorations, themes, errors, search sources and
//! the host-owned native file dialogs.

use crate::files;
/// Generic plugin process bridge. Deliberately runs on the blocking pool:
/// stdout/IPC must never block the WebView or Tauri's async scheduler.
/// Errors that are normal before activation must not flood the diagnostic log.
fn expected_plugin_error(message: &str) -> bool {
    [
        "plugin is not installed",
        "plugin disabled or not approved",
        "plugin is not enabled",
        "permission not granted",
        "not authorized",
    ]
    .iter()
    .any(|prefix| message.to_ascii_lowercase().contains(prefix))
}
fn record_plugin_error(plugin_id: &str, source: &str, message: &str) {
    if expected_plugin_error(message) {
        return;
    }
    let _ = crate::plugin_errors::report(plugin_id, source, message, None);
}
#[tauri::command]
pub(crate) async fn plugin_invoke(
    plugin_id: String,
    method: String,
    params: serde_json::Value,
    directory: Option<String>,
    selected_paths: Option<Vec<String>>,
) -> Result<serde_json::Value, String> {
    let id = plugin_id.clone();
    let call = method.clone();
    tauri::async_runtime::spawn_blocking(move || match (directory, selected_paths) {
        (Some(directory), Some(paths)) => crate::plugin_host::global().invoke_selected(
            &plugin_id,
            &method,
            params,
            &directory,
            &paths
                .into_iter()
                .map(std::path::PathBuf::from)
                .collect::<Vec<_>>(),
        ),
        (Some(directory), None) => crate::plugin_host::global().invoke_selected(
            &plugin_id,
            &method,
            params,
            &directory,
            &[],
        ),
        (None, Some(_)) => Err("selected paths require an operation directory".into()),
        (None, None) => crate::plugin_host::global().invoke(&plugin_id, &method, params),
    })
    .await
    .map_err(|e| e.to_string())?
    .inspect_err(|error| record_plugin_error(&id, "plugin", &format!("{call}: {error}")))
}
#[tauri::command]
pub(crate) async fn plugin_marketplace(
    repository: Option<String>,
) -> Result<Vec<crate::plugin_installer::MarketplacePlugin>, String> {
    let repository = repository
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "kevinhuang001/fiex-official-plugins".into());
    tauri::async_runtime::spawn_blocking(move || crate::plugin_installer::marketplace(&repository))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub(crate) async fn plugin_release_assets(repository: String) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::plugin_installer::release_assets(&repository)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub(crate) async fn plugin_prepare_install(
    repository: String,
    asset_name: String,
) -> Result<crate::plugin_installer::InstallPreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::plugin_installer::prepare(&repository, &asset_name)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub(crate) async fn plugin_prepare_local_install(
    path: String,
) -> Result<crate::plugin_installer::InstallPreview, String> {
    tauri::async_runtime::spawn_blocking(move || crate::plugin_installer::prepare_local(&path))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub(crate) async fn plugin_confirm_install(
    ticket: String,
    allow_untrusted: Option<bool>,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::plugin_installer::install(&ticket, allow_untrusted.unwrap_or(false))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub(crate) fn plugin_cancel_install(ticket: String) -> Result<(), String> {
    crate::plugin_installer::cancel(&ticket)
}
#[tauri::command]
pub(crate) async fn plugin_uninstall(plugin_id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || crate::plugin_installer::uninstall(&plugin_id))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub(crate) fn plugin_activate(
    plugin_id: String,
    consent: crate::plugin_permissions::ActivationConsent,
) -> Result<(), String> {
    crate::plugin_host::activate(&plugin_id, &consent)
        .inspect_err(|error| record_plugin_error(&plugin_id, "permission", error))
}
#[tauri::command]
pub(crate) fn plugin_set_enabled(plugin_id: String, enabled: bool) -> Result<(), String> {
    crate::plugin_host::set_enabled(&plugin_id, enabled)
}
#[tauri::command]
pub(crate) fn plugin_set_permission(
    plugin_id: String,
    permission: String,
    granted: bool,
) -> Result<(), String> {
    crate::plugin_host::set_permission(&plugin_id, &permission, granted)
}
#[tauri::command]
pub(crate) fn plugin_set_command(
    plugin_id: String,
    command: String,
    granted: bool,
) -> Result<(), String> {
    crate::plugin_host::set_command(&plugin_id, &command, granted)
}
#[tauri::command]
pub(crate) fn plugin_set_network_domain(
    plugin_id: String,
    domain: String,
    granted: bool,
) -> Result<(), String> {
    crate::plugin_host::set_network_domain(&plugin_id, &domain, granted)
}
#[tauri::command]
pub(crate) fn plugin_approve_install(plugin_id: String) -> Result<(), String> {
    crate::plugin_host::approve_install(&plugin_id)
}
#[tauri::command]
pub(crate) async fn file_info_details(path: String) -> Vec<crate::plugin_info::InfoSection> {
    tauri::async_runtime::spawn_blocking(move || crate::plugin_info::file_info(&path))
        .await
        .unwrap_or_default()
}
#[tauri::command]
pub(crate) fn plugin_task(
    plugin_id: String,
    task_id: String,
) -> Option<crate::plugin_host::PluginTaskInfo> {
    crate::plugin_host::global()
        .task(&task_id)
        .filter(|task| task.plugin_id == plugin_id)
}
#[tauri::command]
pub(crate) fn plugin_cancel_task(plugin_id: String, task_id: String) -> Result<(), String> {
    let task = crate::plugin_host::global()
        .task(&task_id)
        .filter(|task| task.plugin_id == plugin_id)
        .ok_or("background task not found")?;
    crate::plugin_host::global().cancel_task(&task.task_id)
}
#[tauri::command]
pub(crate) fn plugin_status() -> Vec<(String, bool)> {
    crate::plugin_host::catalog()
        .into_iter()
        .map(|item| (item.id, item.backend_available))
        .collect()
}
#[tauri::command]
pub(crate) async fn plugin_decorations(
    directory: String,
    entries: Vec<crate::plugin_decorations::DecorationEntry>,
) -> Result<Vec<crate::plugin_decorations::Decoration>, String> {
    if !directory.starts_with('/') && !directory.contains(":\\") && !directory.contains("://") {
        return Err("invalid decoration directory".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        crate::plugin_decorations::query(&directory, entries)
    })
    .await
    .map_err(|error| error.to_string())
}
#[tauri::command]
pub(crate) fn plugin_themes() -> Vec<crate::plugin_themes::Theme> {
    crate::plugin_host::themes()
}
#[tauri::command]
pub(crate) fn plugin_network_domains(plugin_id: String) -> Result<Vec<String>, String> {
    crate::plugin_host::network_domains(&plugin_id)
}
#[tauri::command]
pub(crate) fn plugin_permission_allowed(
    plugin_id: String,
    permission: String,
) -> Result<bool, String> {
    crate::plugin_host::permission_allowed(&plugin_id, &permission)
}
#[tauri::command]
pub(crate) fn plugin_panel_html(plugin_id: String, panel_id: String) -> Result<String, String> {
    crate::plugin_host::panel_html(&plugin_id, &panel_id).inspect_err(|error| {
        record_plugin_error(&plugin_id, "panel", &format!("{panel_id}: {error}"))
    })
}
#[tauri::command]
pub(crate) fn plugin_panel_asset(plugin_id: String, panel_id: String) -> Result<String, String> {
    crate::plugin_host::panel_asset(&plugin_id, &panel_id).inspect_err(|error| {
        record_plugin_error(&plugin_id, "panel", &format!("{panel_id}: {error}"))
    })
}
#[tauri::command]
pub(crate) fn plugins_list() -> Vec<crate::plugin_host::PluginInfo> {
    crate::plugin_host::catalog()
}
#[tauri::command]
pub(crate) fn plugin_errors(plugin_id: String) -> Vec<crate::plugin_errors::PluginError> {
    crate::plugin_errors::list(&plugin_id)
}
#[tauri::command]
pub(crate) fn plugin_error_report(
    plugin_id: String,
    source: String,
    message: String,
    detail: Option<String>,
) -> Result<(), String> {
    crate::plugin_errors::report(&plugin_id, &source, &message, detail.as_deref())
}
#[tauri::command]
pub(crate) fn plugin_error_clear(plugin_id: String) {
    crate::plugin_errors::clear(&plugin_id);
}
#[tauri::command]
pub(crate) fn plugin_storage_get(
    plugin_id: String,
    scope: String,
    key: String,
) -> Result<Option<serde_json::Value>, String> {
    crate::plugin_storage::get(&plugin_id, &scope, &key)
}
#[tauri::command]
pub(crate) fn plugin_storage_set(
    plugin_id: String,
    scope: String,
    key: String,
    value: serde_json::Value,
) -> Result<(), String> {
    crate::plugin_storage::set(&plugin_id, &scope, &key, value)
}
#[tauri::command]
pub(crate) fn plugin_storage_delete(
    plugin_id: String,
    scope: String,
    key: String,
) -> Result<(), String> {
    crate::plugin_storage::delete(&plugin_id, &scope, &key)
}
#[tauri::command]
pub(crate) fn plugin_secret_set(
    plugin_id: String,
    scope: String,
    key: String,
    value: String,
) -> Result<(), String> {
    let permitted = crate::plugin_host::catalog()
        .into_iter()
        .find(|p| p.id == plugin_id)
        .and_then(|p| p.contributions.get("settings").cloned())
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
    crate::plugin_storage::secret_set(&plugin_id, &scope, &key, &value)
}
#[tauri::command]
pub(crate) fn plugin_secret_available() -> bool {
    crate::plugin_storage::secret_available()
}

#[tauri::command]
pub(crate) async fn plugin_search(
    request: crate::plugin_search::PluginSearchInput,
) -> Result<Vec<files::Entry>, String> {
    tauri::async_runtime::spawn_blocking(move || Ok(crate::plugin_search::query(&request)))
        .await
        .map_err(|e| e.to_string())?
}

/// Host-owned native file chooser. Returns only a selected path; plugin
/// code never gets a handle to the chooser UI or a directory browsing grant.
#[tauri::command]
pub(crate) fn plugin_file_dialog(
    mode: String,
    directory: Option<String>,
    suggested_name: Option<String>,
    extensions: Option<Vec<String>>,
) -> Result<Option<String>, String> {
    let mut dialog = rfd::FileDialog::new();
    if let Some(path) = directory.filter(|v| std::path::Path::new(v).is_dir()) {
        dialog = dialog.set_directory(path);
    }
    if let Some(name) = suggested_name {
        let path = std::path::Path::new(&name);
        if path.file_name().is_none()
            || path.components().count() != 1
            || name.contains(['\\', '/', '\0'])
        {
            return Err("invalid suggested filename".into());
        }
        dialog = dialog.set_file_name(name);
    }
    if let Some(exts) = extensions {
        if exts.len() > 20
            || exts.iter().any(|v| {
                v.is_empty() || v.len() > 16 || !v.chars().all(|c| c.is_ascii_alphanumeric())
            })
        {
            return Err("invalid file extension filter".into());
        }
        if !exts.is_empty() {
            let refs = exts.iter().map(String::as_str).collect::<Vec<_>>();
            dialog = dialog.add_filter("文件类型", &refs);
        }
    }
    let selection = match mode.as_str() {
        "openFile" => dialog.pick_file(),
        "openDirectory" => dialog.pick_folder(),
        "saveFile" => dialog.save_file(),
        _ => return Err("unsupported file picker mode".into()),
    };
    Ok(selection.map(|p| p.to_string_lossy().into_owned()))
}
#[tauri::command]
pub(crate) fn plugin_files_dialog(
    directory: Option<String>,
    extensions: Option<Vec<String>>,
) -> Result<Option<Vec<String>>, String> {
    let mut dialog = rfd::FileDialog::new();
    if let Some(path) = directory.filter(|v| std::path::Path::new(v).is_dir()) {
        dialog = dialog.set_directory(path);
    }
    if let Some(exts) = extensions {
        if exts.len() > 20
            || exts.iter().any(|v| {
                v.is_empty() || v.len() > 16 || !v.chars().all(|c| c.is_ascii_alphanumeric())
            })
        {
            return Err("invalid extension filter".into());
        }
        if !exts.is_empty() {
            let refs = exts.iter().map(String::as_str).collect::<Vec<_>>();
            dialog = dialog.add_filter("文件类型", &refs);
        }
    }
    Ok(dialog.pick_files().map(|paths| {
        paths
            .into_iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect()
    }))
}
