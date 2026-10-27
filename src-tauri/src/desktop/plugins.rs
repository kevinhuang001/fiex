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
        "permissiondenied",
        "permission request",
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
    preview_session_id: Option<String>,
    resource_bindings: Option<Vec<crate::plugin_resource_context::ResourceBinding>>,
) -> Result<serde_json::Value, String> {
    if crate::plugin_events::is_protected_method(&method) {
        return Err("PermissionDenied: provider and host methods are Core-only".into());
    }
    let mut bindings = resource_bindings.unwrap_or_default();
    let mut directory = directory;
    let mut selected_paths = selected_paths;
    let mut params = params;
    // Selected virtual resources become exact Core-owned materializations.
    // This never grants a feature plugin access to the owner's repository.
    if let Some(paths) = selected_paths.as_mut() {
        let mut replacements = std::collections::HashMap::new();
        for path in paths.iter_mut() {
            if crate::mounts::handles(path) {
                let local = materialize_file(path.clone()).await?;
                replacements.insert(path.clone(), local.clone());
                *path = local;
            }
        }
        if !replacements.is_empty() {
            if let Some(original) = directory.as_ref().filter(|p| crate::mounts::handles(p)) {
                let local = std::path::Path::new(&paths[0])
                    .parent()
                    .ok_or("invalid materialization path")?
                    .to_string_lossy()
                    .into_owned();
                replacements.insert(original.clone(), local.clone());
                directory = Some(local);
            }
            fn replace(
                value: &mut serde_json::Value,
                mappings: &std::collections::HashMap<String, String>,
            ) {
                match value {
                    serde_json::Value::String(text) => {
                        if let Some(local) = mappings.get(text) {
                            *text = local.clone();
                        }
                    }
                    serde_json::Value::Array(items) => {
                        for item in items {
                            replace(item, mappings);
                        }
                    }
                    serde_json::Value::Object(fields) => {
                        for item in fields.values_mut() {
                            replace(item, mappings);
                        }
                    }
                    _ => {}
                }
            }
            replace(&mut params, &replacements);
            for binding in &mut bindings {
                if let Some(local) = replacements.get(&binding.path) {
                    binding.path = local.clone();
                }
            }
        }
    }
    let preview_scope = preview_session_id
        .as_deref()
        .map(|id| crate::plugin_preview::session_scope(id, &plugin_id))
        .transpose()?;
    let id = plugin_id.clone();
    let call = method.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(mut scope) = preview_scope {
            crate::plugin_resource_context::bind_selected(&mut scope, &bindings)?;
            return crate::plugin_host::global().invoke_scoped(
                &plugin_id,
                &method,
                params,
                Some(scope),
            );
        }
        match (directory, selected_paths) {
            (Some(directory), Some(paths)) => crate::plugin_host::global()
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
            (Some(directory), None) => crate::plugin_host::global().invoke_selected(
                &plugin_id,
                &method,
                params,
                &directory,
                &[],
            ),
            (None, Some(_)) => Err("selected paths require an operation directory".into()),
            (None, None) => crate::plugin_host::global().invoke(&plugin_id, &method, params),
        }
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
pub(crate) async fn plugin_activate(
    plugin_id: String,
    consent: crate::plugin_permissions::ActivationConsent,
) -> Result<(), String> {
    // Activation verifies the package and starts the resident backend, which
    // may touch the filesystem. Keep that off the WebView thread.
    tauri::async_runtime::spawn_blocking(move || {
        crate::plugin_host::activate(&plugin_id, &consent)
            .inspect_err(|error| record_plugin_error(&plugin_id, "permission", error))
    })
    .await
    .map_err(|error| error.to_string())?
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
/// The UI answers a Core-owned runtime permission prompt. The plugin never
/// participates in this path.
#[tauri::command]
pub(crate) fn plugin_permission_respond(request_id: String, allow: bool) -> Result<(), String> {
    crate::plugin_permission_prompt::respond(&request_id, allow)
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
pub(crate) fn plugin_set_network_all(plugin_id: String, all: bool) -> Result<(), String> {
    crate::plugin_host::set_network_all(&plugin_id, all)
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
    tauri::async_runtime::spawn_blocking(move || {
        crate::plugin_info::file_info_resource(&path).unwrap_or_default()
    })
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
    include_plugins: Option<bool>,
) -> Result<Vec<crate::plugin_decorations::Decoration>, String> {
    if !directory.starts_with('/') && !directory.contains(":\\") && !directory.contains("://") {
        return Err("invalid decoration directory".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        crate::plugin_decorations::query_with_plugins(
            &directory,
            entries,
            include_plugins.unwrap_or(true),
        )
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
            dialog = dialog.add_filter(crate::i18n::t("文件类型"), &refs);
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
            dialog = dialog.add_filter(crate::i18n::t("文件类型"), &refs);
        }
    }
    Ok(dialog.pick_files().map(|paths| {
        paths
            .into_iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect()
    }))
}

/// One window reports all live history references and explicitly saved paths.
#[tauri::command]
pub(crate) fn vfs_retain_views(
    window: tauri::Window,
    paths: Vec<String>,
    durable: Vec<String>,
) -> Result<(), String> {
    crate::mounts::retain(window.label(), &paths, &durable)
}
#[tauri::command]
pub(crate) async fn materialize_file(path: String) -> Result<String, String> {
    if crate::remote::is_remote(&path) {
        if crate::remote::stat(&path)?.is_dir {
            crate::remote::download_dir(&path, &crate::remote::TransferOptions::default()).await?;
            return crate::remote::cached_path(&path);
        }
        return crate::remote::ensure_downloaded(&path, &crate::remote::TransferOptions::default())
            .await;
    }
    if crate::plugin_vfs::handles(&path) {
        return tauri::async_runtime::spawn_blocking(move || crate::plugin_vfs::materialize(&path))
            .await
            .map_err(|e| e.to_string())?;
    }
    crate::mounts::reject_local(&path)?;
    Ok(path)
}

/// Only the owner of an existing view may recover its already authorized
/// resource context. Other plugins see the URI, never the backing local root.
#[tauri::command]
pub(crate) fn plugin_context_directory(plugin_id: String, path: String) -> Result<String, String> {
    if !crate::mounts::handles(&path) {
        return Ok(path);
    }
    let (_, source) = crate::mounts::resolve(&path)?;
    match source {
        crate::mounts::Source::View(view) if view.descriptor.plugin_id == plugin_id => {
            Ok(view.descriptor.resource_root)
        }
        _ => Ok(path),
    }
}

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
            .map(|p| crate::plugin_resource_context::ResourceSnapshot::resolve(p))
            .collect::<Result<Vec<_>, _>>()?;
        crate::plugin_events::call_owner(
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
        use crate::plugin_events::{EventRouter, Handler};
        let resource = crate::plugin_resource_context::ResourceSnapshot::resolve(&path)?;
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
                crate::plugin_events::invoke_resources(
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
pub(crate) fn preview_candidates(
    extension: String,
    mime: String,
    is_dir: bool,
) -> Vec<serde_json::Value> {
    crate::plugin_preview::candidates_json(&extension, &mime, is_dir)
}

/// Optional package-local loading document for a plugin preview. It contains
/// no preview session and grants no resource access; the frontend renders it in
/// the same sandboxed Core-owned iframe while `preview.open` runs.
#[tauri::command]
pub(crate) fn preview_loading(plugin_id: String) -> Result<Option<String>, String> {
    crate::plugin_host::preview_loading(&plugin_id).inspect_err(|error| {
        record_plugin_error(&plugin_id, "preview", &format!("loading: {error}"))
    })
}

#[tauri::command]
pub(crate) fn preview_loading_asset(plugin_id: String) -> Result<Option<String>, String> {
    crate::plugin_host::preview_loading_asset(&plugin_id).inspect_err(|error| {
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
        crate::plugin_preview::open_resource(
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
) -> Result<Vec<serde_json::Value>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::plugin_context_menu::query(&directory, &paths)
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
        crate::plugin_context_menu::children(&menu_session_id, &parent_id)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn plugin_context_menu_execute(
    menu_session_id: String,
    item_id: String,
    on_progress: Option<tauri::ipc::Channel<super::files::OperationProgress>>,
    cancel: tauri::State<'_, crate::operations::OperationCancel>,
) -> Result<serde_json::Value, String> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        crate::plugin_context_menu::execute(&menu_session_id, &item_id)
    })
    .await
    .map_err(|error| error.to_string())??;
    if result["kind"] == "remoteAction" {
        let progress = on_progress.map(super::files::byte_progress_emitter);
        let options = crate::remote::TransferOptions {
            cancel: Some(cancel.flag()),
            progress: progress.map(|emit| {
                std::sync::Arc::new(move |done, total| emit(done, total, None))
                    as std::sync::Arc<crate::remote::ProgressFn>
            }),
        };
        crate::plugin_context_menu::execute_remote(&result, &options).await?;
        return Ok(serde_json::json!({"handled":true}));
    }
    Ok(result)
}

#[tauri::command]
pub(crate) async fn plugin_describe_resources(
    plugin_id: String,
    paths: Vec<String>,
) -> Result<Vec<crate::plugin_contracts::ResourceContext>, String> {
    if paths.len() > 256 {
        return Err("too many resource candidates".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let plugin = crate::plugin_host::catalog()
            .into_iter()
            .find(|plugin| plugin.id == plugin_id && plugin.approved && plugin.enabled)
            .ok_or("plugin unavailable")?;
        let declared = plugin.declared_permissions.iter().cloned().collect();
        let policy = crate::plugin_permission_policy::PermissionPolicy::new(&plugin_id, &declared);
        paths
            .iter()
            .map(|path| {
                Ok(
                    crate::plugin_resource_context::ResourceSnapshot::resolve(path)?.project(
                        &plugin_id,
                        "",
                        policy.allowed(crate::plugin_permission_policy::Capability::Metadata),
                        false,
                    ),
                )
            })
            .collect()
    })
    .await
    .map_err(|error| error.to_string())?
}
