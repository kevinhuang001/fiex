use super::*;
/// Generic plugin process bridge. Deliberately runs on the blocking pool:
/// stdout/IPC must never block the WebView or Tauri's async scheduler.
/// Errors that are normal before activation must not flood the diagnostic log.
pub(super) fn expected_plugin_error(message: &str) -> bool {
    [
        "plugin is not installed",
        "plugin disabled or not approved",
        "plugin is not enabled",
        "permission not granted",
        "permissiondenied",
        "permission request",
        "not authorized",
    ]
    .iter()
    .any(|prefix| message.to_ascii_lowercase().contains(prefix))
}

pub(super) fn record_plugin_error(plugin_id: &str, source: &str, message: &str) {
    if expected_plugin_error(message) {
        return;
    }
    let _ = crate::plugins::errors::report(plugin_id, source, message, None);
}

#[tauri::command]
pub(crate) async fn plugin_invoke(
    plugin_id: String,
    method: String,
    params: serde_json::Value,
    directory: Option<String>,
    selected_paths: Option<Vec<String>>,
    preview_session_id: Option<String>,
    resource_bindings: Option<Vec<crate::plugins::resource::resource_context::ResourceBinding>>,
) -> Result<serde_json::Value, String> {
    if crate::plugins::events::is_protected_method(&method) {
        return Err("PermissionDenied: provider and host methods are Core-only".into());
    }
    let bindings = resource_bindings.unwrap_or_default();
    let preview_scope = preview_session_id
        .as_deref()
        .map(|id| crate::plugins::resolvers::preview::session_scope(id, &plugin_id))
        .transpose()?;
    let id = plugin_id.clone();
    let call = method.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(mut scope) = preview_scope {
            crate::plugins::resource::resource_context::bind_selected(&mut scope, &bindings)?;
            return crate::plugins::host::global().invoke_scoped(
                &plugin_id,
                &method,
                params,
                Some(scope),
            );
        }
        match (directory, selected_paths) {
            (Some(directory), Some(paths)) => crate::plugins::host::global()
                .invoke_selected_with_bindings(
                    &plugin_id,
                    &method,
                    params,
                    &directory,
                    &paths
                        .into_iter()
                        .map(std::path::PathBuf::from)
                        .collect::<Vec<_>>(),
                    &bindings,
                ),
            (Some(directory), None) => crate::plugins::host::global().invoke_selected(
                &plugin_id,
                &method,
                params,
                &directory,
                &[],
            ),
            (None, Some(_)) => Err("selected paths require an operation directory".into()),
            (None, None) => crate::plugins::host::global().invoke(&plugin_id, &method, params),
        }
    })
    .await
    .map_err(|e| e.to_string())?
    .inspect_err(|error| record_plugin_error(&id, "plugin", &format!("{call}: {error}")))
}
