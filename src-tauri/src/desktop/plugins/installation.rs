use super::*;

#[tauri::command]
pub(crate) async fn plugin_marketplace(
    repository: Option<String>,
) -> Result<Vec<crate::plugins::installer::MarketplacePlugin>, String> {
    let repository = repository
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "kevinhuang001/fiex-official-plugins".into());
    tauri::async_runtime::spawn_blocking(move || {
        crate::plugins::installer::marketplace(&repository)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) async fn plugin_release_assets(repository: String) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::plugins::installer::release_assets(&repository)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) async fn plugin_prepare_install(
    repository: String,
    asset_name: String,
) -> Result<crate::plugins::installer::InstallPreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::plugins::installer::prepare(&repository, &asset_name)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) async fn plugin_prepare_local_install(
    path: String,
) -> Result<crate::plugins::installer::InstallPreview, String> {
    tauri::async_runtime::spawn_blocking(move || crate::plugins::installer::prepare_local(&path))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) async fn plugin_confirm_install(
    ticket: String,
    allow_untrusted: Option<bool>,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::plugins::installer::install(&ticket, allow_untrusted.unwrap_or(false))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) fn plugin_cancel_install(ticket: String) -> Result<(), String> {
    crate::plugins::installer::cancel(&ticket)
}

#[tauri::command]
pub(crate) async fn plugin_uninstall(plugin_id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || crate::plugins::installer::uninstall(&plugin_id))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) async fn plugin_activate(
    plugin_id: String,
    consent: crate::plugins::security::permissions::ActivationConsent,
) -> Result<(), String> {
    // Activation verifies the package and starts the resident backend, which
    // may touch the filesystem. Keep that off the WebView thread.
    tauri::async_runtime::spawn_blocking(move || {
        crate::plugins::host::activate(&plugin_id, &consent)
            .inspect_err(|error| record_plugin_error(&plugin_id, "permission", error))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) fn plugin_set_enabled(plugin_id: String, enabled: bool) -> Result<(), String> {
    crate::plugins::host::set_enabled(&plugin_id, enabled)
}

#[tauri::command]
pub(crate) fn plugin_set_permission(
    plugin_id: String,
    permission: String,
    granted: bool,
) -> Result<(), String> {
    crate::plugins::host::set_permission(&plugin_id, &permission, granted)
}
/// The UI answers a Core-owned runtime permission prompt. The plugin never
/// participates in this path.
#[tauri::command]
pub(crate) fn plugin_permission_respond(request_id: String, allow: bool) -> Result<(), String> {
    crate::plugins::security::prompt::respond(&request_id, allow)
}

#[tauri::command]
pub(crate) fn plugin_set_command(
    plugin_id: String,
    command: String,
    granted: bool,
) -> Result<(), String> {
    crate::plugins::host::set_command(&plugin_id, &command, granted)
}

#[tauri::command]
pub(crate) fn plugin_set_network_all(plugin_id: String, all: bool) -> Result<(), String> {
    crate::plugins::host::set_network_all(&plugin_id, all)
}

#[tauri::command]
pub(crate) fn plugin_set_network_domain(
    plugin_id: String,
    domain: String,
    granted: bool,
) -> Result<(), String> {
    crate::plugins::host::set_network_domain(&plugin_id, &domain, granted)
}

#[tauri::command]
pub(crate) fn plugin_approve_install(plugin_id: String) -> Result<(), String> {
    crate::plugins::host::approve_install(&plugin_id)
}
