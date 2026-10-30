/// Generic plugin process bridge. Deliberately runs on the blocking pool:
/// stdout/IPC must never block the WebView or Tauri's async scheduler.
/// Errors that are normal before activation must not flood the diagnostic log.
pub(super) fn record_plugin_error(plugin_id: &str, source: &str, message: &str) {
    let _ = crate::infrastructure::logging::report(plugin_id, source, message, None);
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
    let scope = directory.clone().unwrap_or_else(|| "-".into());
    let started = std::time::Instant::now();
    crate::infrastructure::logging::record_core(
        "debug",
        "plugin.invoke",
        &format!("{id}: {call} started (directory={scope})"),
    );
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
    .inspect(|_| {
        crate::infrastructure::logging::record_core(
            "debug",
            "plugin.invoke",
            &format!(
                "{id}: {call} completed in {} ms (directory={scope})",
                started.elapsed().as_millis()
            ),
        );
        crate::plugins::errors::report_log_limited(
            &id,
            "debug",
            &format!("{call} completed in {} ms", started.elapsed().as_millis()),
        );
    })
    .inspect_err(|error| {
        record_plugin_error(&id, "plugin", &format!("{call}: {error}"));
        crate::infrastructure::logging::record_core(
            "error",
            "plugin.invoke",
            &format!(
                "{id}: {call} failed after {} ms (directory={scope})",
                started.elapsed().as_millis()
            ),
        );
    })
}
