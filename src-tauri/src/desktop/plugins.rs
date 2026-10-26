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
) -> Result<serde_json::Value, String> {
    if crate::plugin_events::is_protected_method(&method) {
        return Err("PermissionDenied: provider and host methods are Core-only".into());
    }
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
        }
    }
    let preview_scope = preview_session_id
        .as_deref()
        .map(|id| crate::plugin_preview::session_scope(id, &plugin_id))
        .transpose()?;
    let id = plugin_id.clone();
    let call = method.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(scope) = preview_scope {
            return crate::plugin_host::global().invoke_scoped(
                &plugin_id,
                &method,
                params,
                Some(scope),
            );
        }
        match (directory, selected_paths) {
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

type ContextMenuRegistry = std::collections::HashMap<String, ContextMenuSession>;
struct ContextMenuSession {
    plugin_id: String,
    directory: String,
    resources: Vec<crate::plugin_resource_context::ResourceSnapshot>,
    actions: std::collections::HashSet<String>,
    submenus: std::collections::HashMap<String, usize>,
    created: std::time::Instant,
}
fn context_menu_sessions() -> &'static std::sync::Mutex<ContextMenuRegistry> {
    static SESSIONS: std::sync::OnceLock<std::sync::Mutex<ContextMenuRegistry>> =
        std::sync::OnceLock::new();
    SESSIONS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}
fn menu_allowed(plugin_id: &str) -> bool {
    crate::plugin_events::EventRouter::providers("contextMenu.resolve",&[],"",Vec::new()).iter().any(|p|matches!(&p.handler,crate::plugin_events::Handler::Plugin(plugin) if plugin.id==plugin_id))
}
fn menu_response_valid(
    result: &serde_json::Value,
) -> Result<std::collections::HashSet<String>, String> {
    use std::collections::{HashMap, HashSet};
    let items = result
        .get("items")
        .and_then(serde_json::Value::as_array)
        .ok_or("menu response requires items[]")?;
    if items.len() > 100 {
        return Err("too many menu items".into());
    }
    let mut ids = HashSet::new();
    let mut kinds = HashMap::new();
    let mut parents = HashMap::new();
    let mut actions = HashSet::new();
    for item in items {
        let id = item
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or("menu item id missing")?;
        if id.is_empty()
            || id.len() > 80
            || !id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
            || !ids.insert(id.to_owned())
        {
            return Err("invalid or duplicate menu item id".into());
        }
        let kind = item
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .ok_or("menu kind missing")?;
        if !matches!(kind, "action" | "submenu" | "separator") {
            return Err("invalid menu item kind".into());
        }
        let title = item
            .get("title")
            .and_then(serde_json::Value::as_str)
            .ok_or("menu title missing")?;
        if title.is_empty() || title.len() > 128 {
            return Err("invalid menu item title".into());
        }
        if item.get("when").is_some() || item.get("children").is_some() {
            return Err("static nested menus are forbidden".into());
        }
        if let Some(parent) = item.get("parentId") {
            parents.insert(
                id.to_owned(),
                parent.as_str().ok_or("invalid menu parent")?.to_owned(),
            );
        }
        if kind == "action" {
            actions.insert(id.to_owned());
        }
        kinds.insert(id.to_owned(), kind.to_owned());
    }
    for id in &ids {
        let mut current = id.as_str();
        let mut depth = 1;
        let mut seen = HashSet::new();
        while let Some(parent) = parents.get(current) {
            if !seen.insert(current) || kinds.get(parent).map(String::as_str) != Some("submenu") {
                return Err("menu contains a cycle or invalid parent".into());
            }
            depth += 1;
            if depth > 4 {
                return Err("menu nesting exceeds depth limit".into());
            }
            current = parent;
        }
    }
    Ok(actions)
}
/// Track submenu depth so a lazy expansion cannot exceed the four-level cap.
fn root_menu_valid(
    result: &serde_json::Value,
) -> Result<std::collections::HashSet<String>, String> {
    let items = result
        .get("items")
        .and_then(serde_json::Value::as_array)
        .ok_or("menu response requires items[]")?;
    if items.iter().any(|item| item.get("parentId").is_some()) {
        return Err("root menu must not include submenu children".into());
    }
    menu_response_valid(result)
}

fn submenu_depths(result: &serde_json::Value) -> std::collections::HashMap<String, usize> {
    use std::collections::HashMap;
    let Some(items) = result.get("items").and_then(serde_json::Value::as_array) else {
        return HashMap::new();
    };
    let by_id: HashMap<&str, &serde_json::Value> = items
        .iter()
        .filter_map(|item| {
            item.get("id")
                .and_then(serde_json::Value::as_str)
                .map(|id| (id, item))
        })
        .collect();
    let mut depths = HashMap::new();
    for item in items {
        if item.get("kind").and_then(serde_json::Value::as_str) != Some("submenu") {
            continue;
        }
        let Some(id) = item.get("id").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let mut depth = 1;
        let mut current = item;
        while let Some(parent) = current.get("parentId").and_then(serde_json::Value::as_str) {
            depth += 1;
            let Some(ancestor) = by_id.get(parent) else {
                break;
            };
            current = ancestor;
        }
        depths.insert(id.to_string(), depth);
    }
    depths
}

/// Validate each lazy submenu response independently. The plugin cannot
/// supply the parent id or forge actions from another menu session.
struct ValidatedSubmenu {
    items: Vec<serde_json::Value>,
    actions: std::collections::HashSet<String>,
    submenus: std::collections::HashMap<String, usize>,
}

fn validated_submenu(
    value: &serde_json::Value,
    parent_id: &str,
    parent_depth: usize,
) -> Result<ValidatedSubmenu, String> {
    let children = value
        .get("items")
        .and_then(serde_json::Value::as_array)
        .ok_or("submenu response requires items[]")?;
    if children.len() > 99 || (parent_depth >= 4 && !children.is_empty()) {
        return Err("submenu exceeds item/depth limit".into());
    }
    let mut items = Vec::with_capacity(children.len() + 1);
    items.push(serde_json::json!({"id":parent_id,"kind":"submenu","title":"parent"}));
    for child in children {
        let mut child = child
            .as_object()
            .ok_or("submenu entry must be an object")?
            .clone();
        if child.contains_key("parentId") {
            return Err("submenu children must not supply parentId".into());
        }
        child.insert("parentId".into(), serde_json::json!(parent_id));
        items.push(serde_json::Value::Object(child));
    }
    let response = serde_json::json!({"items":items});
    let actions = menu_response_valid(&response)?;
    let submenus = submenu_depths(&response)
        .into_iter()
        .filter(|(id, _)| id != parent_id)
        .map(|(id, _)| (id, parent_depth + 1))
        .collect();
    Ok(ValidatedSubmenu {
        items: items.into_iter().skip(1).collect(),
        actions,
        submenus,
    })
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
    _is_directory: bool,
    _read_only: bool,
    _is_remote: bool,
) -> Result<Vec<serde_json::Value>, String> {
    tauri::async_runtime::spawn_blocking(move ||{
        use crate::plugin_events::{EventRouter,Handler};
        if paths.is_empty()||paths.len()>256||directory.is_empty(){return Err("invalid menu selection".into());}
        let resources=paths.iter().map(|p|crate::plugin_resource_context::ResourceSnapshot::resolve(p)).collect::<Result<Vec<_>,_>>()?;
        let metadata=resources.iter().map(|r|serde_json::to_value(r.metadata()).unwrap_or_default()).collect::<Vec<_>>();
        let providers=EventRouter::providers("contextMenu.resolve",&metadata,&directory,Vec::new());
        Ok(EventRouter::resolve("contextMenu.resolve",providers,|_,_|Err("no builtin menu registered".into()),
            |plugin|{
                let session_id=crate::mounts::new_id()?;
                let mut result=crate::plugin_events::invoke_resources(plugin,"contextMenu.resolve",serde_json::json!({"menuSessionId":session_id,"interactionId":crate::mounts::new_id()?}),&resources,Some(&directory))?;
                result["menuSessionId"]=serde_json::json!(session_id);Ok(result)
            },
            |provider,result|{
                let Handler::Plugin(plugin)=&provider.handler else{return Err("invalid menu provider".into());};
                let actions=root_menu_valid(&result)?;let submenus=submenu_depths(&result);
                let session_id=result["menuSessionId"].as_str().ok_or("missing menu session")?.to_string();
                let mut guard=context_menu_sessions().lock().map_err(|_|"menu registry unavailable")?;
                guard.retain(|_,s|s.created.elapsed().as_secs()<90);
                if guard.len()>=128{return Err("menu session capacity exceeded".into());}
                guard.insert(session_id,ContextMenuSession{plugin_id:plugin.id.clone(),directory:directory.clone(),resources:resources.clone(),actions,submenus,created:std::time::Instant::now()});
                Ok(serde_json::json!({"pluginId":plugin.id,"menuSessionId":result["menuSessionId"],"items":result["items"]}))
            },|_|true,|results|results.into_iter().filter_map(|(_,r)|r.ok()).collect::<Vec<_>>()))
    }).await.map_err(|e|e.to_string())?
}
/// Resolve children on demand whenever a user opens a submenu. No file
/// content grant is attached until the user clicks a validated action.
#[tauri::command]
pub(crate) async fn plugin_context_menu_children(
    menu_session_id: String,
    parent_id: String,
) -> Result<serde_json::Value, String> {
    let (plugin_id, directory, resources, depth) = {
        let guard = context_menu_sessions()
            .lock()
            .map_err(|_| "menu registry unavailable")?;
        let session = guard.get(&menu_session_id).ok_or("menu session expired")?;
        if session.created.elapsed().as_secs() >= 90 || !menu_allowed(&session.plugin_id) {
            return Err("menu session expired".into());
        }
        let depth = *session.submenus.get(&parent_id).ok_or("unknown submenu")?;
        (
            session.plugin_id.clone(),
            session.directory.clone(),
            session.resources.clone(),
            depth,
        )
    };
    tauri::async_runtime::spawn_blocking(move || {
        let params=serde_json::json!({"menuSessionId":menu_session_id,"parentId":parent_id,"interactionId":crate::mounts::new_id()?});
        let result=crate::plugin_events::call_owner("contextMenu.resolve",&plugin_id,params,&resources,Some(&directory))?;
        let validated = validated_submenu(&result, &parent_id, depth)?;
        let mut guard = context_menu_sessions()
            .lock()
            .map_err(|_| "menu registry unavailable")?;
        let session = guard
            .get_mut(&menu_session_id)
            .ok_or("menu session expired")?;
        if session.created.elapsed().as_secs() >= 90 || session.plugin_id != plugin_id {
            return Err("menu session expired".into());
        }
        if validated
            .actions
            .iter()
            .any(|id| session.actions.contains(id) || session.submenus.contains_key(id))
            || validated
                .submenus
                .keys()
                .any(|id| session.actions.contains(id) || session.submenus.contains_key(id))
        {
            return Err("duplicate menu item id".into());
        }
        if session.actions.len()+session.submenus.len()+validated.actions.len()+validated.submenus.len()>512{return Err("menu session item limit exceeded".into());}
        session.actions.extend(validated.actions);
        session.submenus.extend(validated.submenus);
        Ok::<_, String>(serde_json::json!({"items":validated.items}))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn plugin_context_menu_execute(
    menu_session_id: String,
    item_id: String,
) -> Result<serde_json::Value, String> {
    let session = context_menu_sessions()
        .lock()
        .map_err(|_| "menu registry unavailable")?
        .remove(&menu_session_id)
        .ok_or("menu session invalid or already used")?;
    if session.created.elapsed().as_secs() >= 90
        || !session.actions.contains(&item_id)
        || !menu_allowed(&session.plugin_id)
    {
        return Err("menu session expired or item was not offered".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let paths=session.resources.iter().map(crate::plugin_resource_context::ResourceSnapshot::materialize).collect::<Result<Vec<_>,_>>()?;
        let directory=if crate::mounts::handles(&session.directory){paths[0].parent().ok_or("invalid action directory")?.to_string_lossy().into_owned()}else{session.directory.clone()};
        let parameters=serde_json::json!({"menuSessionId":menu_session_id,"itemId":item_id,"interactionId":crate::mounts::new_id()?,"paths":paths,"path":paths[0],"root":directory});
        let mut result=crate::plugin_events::call_owner("contextMenu.execute",&session.plugin_id,parameters,&session.resources,Some(&session.directory))?;
        if result.get("kind").and_then(serde_json::Value::as_str) == Some("panel") {
            let entry = result
                .get("entry")
                .and_then(serde_json::Value::as_str)
                .ok_or("panel result missing entry")?;
            let asset = crate::plugin_host::plugin_view_asset(&session.plugin_id, entry)?;
            let object = result.as_object_mut().ok_or("invalid panel result")?;
            object.remove("entry");
            object.insert("assetPath".into(), serde_json::json!(asset));
        }
        Ok::<serde_json::Value, String>(result)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod context_menu_session_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn menu_rejects_nested_legacy_fields_and_unknown_parent() {
        assert!(menu_response_valid(&json!({"items": [
            {"id":"root","kind":"submenu","title":"Root"},
            {"id":"open","kind":"action","title":"Open","parentId":"root"}
        ]}))
        .unwrap()
        .contains("open"));
        for invalid in [
            json!({"items":[{"id":"a","title":"A","kind":"action","when":"true"}]}),
            json!({"items":[{"id":"a","title":"A","kind":"action","children":[]}]}),
            json!({"items":[{"id":"a","title":"A","kind":"action","parentId":"missing"}]}),
            json!({"items":[{"id":"a","title":"A","kind":"submenu","parentId":"a"}]}),
            json!({"items":[{"id":"a","title":"A","kind":"action"},{"id":"a","title":"B","kind":"action"}]}),
        ] {
            assert!(menu_response_valid(&invalid).is_err());
        }
    }

    #[test]
    fn dynamic_children_require_a_valid_parent_and_cannot_forge_ids() {
        let response = serde_json::json!({
            "items":[{"id":"child","kind":"action","title":"Child"}]
        });
        let validated = validated_submenu(&response, "root", 1).unwrap();
        assert_eq!(validated.items[0]["parentId"], "root");
        assert!(validated.actions.contains("child"));
        assert!(validated.submenus.is_empty());
        assert!(validated_submenu(&response, "root", 4).is_err());
        let spoof = serde_json::json!({"items":[{
            "id":"child","kind":"action","title":"Child","parentId":"other"
        }]});
        assert!(validated_submenu(&spoof, "root", 1).is_err());
    }

    #[test]
    fn root_menu_requires_lazy_submenu_children() {
        let nested = serde_json::json!({"items":[
            {"id":"group","kind":"submenu","title":"Group"},
            {"id":"run","kind":"action","title":"Run","parentId":"group"}
        ]});
        assert!(root_menu_valid(&nested).is_err());
        let root = serde_json::json!({"items":[
            {"id":"group","kind":"submenu","title":"Group"}
        ]});
        assert!(root_menu_valid(&root).unwrap().is_empty());
    }

    #[test]
    fn submenu_cannot_be_executed_as_an_action() {
        let valid = menu_response_valid(&json!({"items":[
            {"id":"group","title":"Group","kind":"submenu"},
            {"id":"act","title":"Act","kind":"action","parentId":"group"}
        ]}))
        .unwrap();
        assert!(!valid.contains("group"));
        assert!(valid.contains("act"));
    }
}
