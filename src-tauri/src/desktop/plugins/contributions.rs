use super::*;

#[tauri::command]
pub(crate) async fn plugin_toolbar_invoke(
    plugin_id: String,
    directory: String,
    paths: Vec<String>,
) -> Result<serde_json::Value, String> {
    if paths.len() > 256 {
        return Err("too many toolbar targets".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let targets = if paths.is_empty() {
            vec![directory.clone()]
        } else {
            paths
        };
        let resources = targets
            .iter()
            .map(|p| crate::plugins::resource::resource_context::ResourceSnapshot::resolve(p))
            .collect::<Result<Vec<_>, _>>()?;
        crate::plugins::events::call_owner(
            "toolbar.invoke",
            &plugin_id,
            serde_json::json!({}),
            &resources,
            Some(&directory),
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) async fn plugin_before_open(path: String) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        use crate::plugins::events::{EventRouter, Handler};
        let resource =
            crate::plugins::resource::resource_context::ResourceSnapshot::resolve(&path)?;
        let metadata = serde_json::to_value(resource.metadata()).map_err(|e| e.to_string())?;
        let providers = EventRouter::providers("hooks.invoke", &[metadata], "", Vec::new())
            .into_iter()
            .filter(|p| {
                let Handler::Plugin(plugin) = &p.handler else {
                    return false;
                };
                plugin.extensions["hooks"]["event"] == "beforeOpen"
            })
            .collect();
        Ok::<_, String>(EventRouter::resolve(
            "hooks.invoke",
            providers,
            |_, _| Err("no builtin hook".into()),
            |plugin| {
                crate::plugins::events::invoke_resources(
                    plugin,
                    "hooks.invoke",
                    serde_json::json!({"event":"beforeOpen"}),
                    std::slice::from_ref(&resource),
                    None,
                )
            },
            |_, value| Ok(value),
            |value| value["handled"] == true,
            |results| {
                results
                    .into_iter()
                    .find_map(|(_, result)| result.ok().filter(|value| value["handled"] == true))
                    .unwrap_or_else(|| serde_json::json!({"handled":false}))
            },
        ))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) async fn preview_candidates(path: String) -> Result<Vec<serde_json::Value>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::plugins::resolvers::preview::resource_candidates_json(&path)
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Optional package-local loading document for a plugin preview. It contains
/// no preview session and grants no resource access; the frontend renders it in
/// the same sandboxed Core-owned iframe while `preview.open` runs.
#[tauri::command]
pub(crate) async fn preview_loading(plugin_id: String) -> Result<Option<String>, String> {
    // Resolving a package document scans installed plugins and reads the file.
    // Keep that I/O off the event loop so the loading page can paint immediately.
    let id = plugin_id.clone();
    tauri::async_runtime::spawn_blocking(move || crate::plugins::host::preview_loading(&id))
        .await
        .map_err(|error| error.to_string())?
        .inspect_err(|error| {
            record_plugin_error(&plugin_id, "preview", &format!("loading: {error}"))
        })
}

#[tauri::command]
pub(crate) async fn preview_loading_asset(plugin_id: String) -> Result<Option<String>, String> {
    // Same bounded disk work as `preview_loading`; the document never starts the
    // plugin process, so the prefetched page is available before `preview.open`.
    let id = plugin_id.clone();
    tauri::async_runtime::spawn_blocking(move || crate::plugins::host::preview_loading_asset(&id))
        .await
        .map_err(|error| error.to_string())?
        .inspect_err(|error| {
            record_plugin_error(&plugin_id, "preview", &format!("loading asset: {error}"))
        })
}

#[tauri::command]
pub(crate) async fn preview_open(
    path: String,
    provider_id: Option<String>,
    provider_order: Option<Vec<String>>,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::plugins::resolvers::preview::open_resource(
            &path,
            provider_id.as_deref(),
            provider_order.as_deref(),
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) async fn plugin_context_menu_resolve(
    directory: String,
    paths: Vec<String>,
    source: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    let source = source.unwrap_or_else(|| "all".into());
    tauri::async_runtime::spawn_blocking(move || match source.as_str() {
        "core" => crate::plugins::resolvers::context_menu::query_core(&directory, &paths),
        "plugins" => crate::plugins::resolvers::context_menu::query_plugins(&directory, &paths),
        "" | "all" => crate::plugins::resolvers::context_menu::query(&directory, &paths),
        other => Err(format!("unknown context menu source: {other}")),
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn plugin_context_menu_children(
    menu_session_id: String,
    parent_id: String,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::plugins::resolvers::context_menu::children(&menu_session_id, &parent_id)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn plugin_context_menu_execute(
    menu_session_id: String,
    item_id: String,
    on_progress: tauri::ipc::Channel<super::super::files::OperationProgress>,
    cancel: tauri::State<'_, crate::core::operations::OperationCancel>,
) -> Result<serde_json::Value, String> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        crate::plugins::resolvers::context_menu::execute(&menu_session_id, &item_id)
    })
    .await
    .map_err(|error| error.to_string())??;
    if result["kind"] == "remoteAction" {
        let progress = Some(super::super::files::byte_progress_emitter(on_progress));
        let options = crate::remote::TransferOptions {
            cancel: Some(cancel.flag()),
            progress: progress.map(|emit| {
                std::sync::Arc::new(move |done, total| emit(done, total, None))
                    as std::sync::Arc<crate::remote::ProgressFn>
            }),
        };
        crate::plugins::resolvers::context_menu::execute_remote(&result, &options).await?;
        return Ok(serde_json::json!({"handled":true}));
    }
    Ok(result)
}

#[tauri::command]
pub(crate) async fn plugin_connection_menu_resolve(
    connection_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::plugins::resolvers::connection_menu::query(&connection_id)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn plugin_connection_menu_execute(
    connection_id: String,
    provider_id: String,
    item_id: String,
    menu_session_id: Option<String>,
) -> Result<serde_json::Value, String> {
    crate::plugins::resolvers::connection_menu::execute(
        &connection_id,
        &provider_id,
        &item_id,
        menu_session_id.as_deref(),
    )
    .await
}

#[tauri::command]
pub(crate) async fn plugin_describe_resources(
    plugin_id: String,
    paths: Vec<String>,
) -> Result<Vec<crate::plugins::contracts::ResourceContext>, String> {
    if paths.len() > 256 {
        return Err("too many resource candidates".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let plugin = crate::plugins::host::catalog()
            .into_iter()
            .find(|plugin| plugin.id == plugin_id && plugin.approved && plugin.enabled)
            .ok_or("plugin unavailable")?;
        let declared = plugin.declared_permissions.iter().cloned().collect();
        let policy = crate::plugins::security::policy::PermissionPolicy::new(&plugin_id, &declared);
        paths
            .iter()
            .map(|path| {
                Ok(
                    crate::plugins::resource::resource_context::ResourceSnapshot::resolve(path)?
                        .project(
                            &plugin_id,
                            "",
                            policy.allowed(crate::plugins::security::policy::Capability::Metadata),
                            false,
                        ),
                )
            })
            .collect()
    })
    .await
    .map_err(|error| error.to_string())?
}
