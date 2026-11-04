use super::*;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
};

type DirectorySizeRequest = (String, Arc<AtomicBool>);

fn directory_size_requests() -> &'static Mutex<HashMap<String, DirectorySizeRequest>> {
    static REQUESTS: OnceLock<Mutex<HashMap<String, DirectorySizeRequest>>> = OnceLock::new();
    REQUESTS.get_or_init(Default::default)
}

#[tauri::command]
pub(crate) async fn file_info_details(
    path: String,
    on_partial: tauri::ipc::Channel<Vec<crate::plugins::providers::info::InfoSection>>,
) -> Result<Vec<crate::plugins::providers::info::InfoSection>, String> {
    use crate::infrastructure::cache::memory::MemoryCache;
    use crate::plugins::providers::info::InfoSection;
    use std::sync::OnceLock;
    use std::time::Duration;
    static INFO: OnceLock<MemoryCache<Vec<InfoSection>>> = OnceLock::new();
    let user_action = crate::core::fs::mapped_access::UserActionContext::from_desktop_command();
    tauri::async_runtime::spawn_blocking(move || {
        let cache = INFO.get_or_init(|| MemoryCache::new(256, Duration::from_secs(90)));
        // A cache lookup must still validate the current resource identity.
        // An InfoProvider can work without vfs.stat, so missing stat only
        // disables caching rather than disabling the Info feature.
        let resource = crate::plugins::resource::resource_context::ResourceSnapshot::resolve_from_directory_snapshot(&path)?;
        let _broker = crate::core::fs::trusted_snapshot(&resource, "core:info")?;
        let metadata = resource.metadata();
        let parent = crate::core::fs::provider_registry::parent_path(&path).unwrap_or_else(|_| path.clone());
        let mount_revision = crate::core::fs::directory_cache::key(&parent);
        let content_revision = crate::core::fs::changes::revision(&parent);
        let provider_state = crate::plugins::host::catalog()
            .into_iter()
            .map(|plugin| {
                (
                    plugin.id,
                    plugin.version,
                    plugin.enabled,
                    plugin.approved,
                    plugin.backend_available,
                    plugin.granted_permissions,
                )
            })
            .collect::<Vec<_>>();
        let key = serde_json::to_string(&(
            &resource.reference,
            metadata,
            resource
                .operations
                .map(|capabilities| capabilities.into_allowed_ops()),
            mount_revision,
            content_revision,
            provider_state,
            crate::i18n::locale(),
            crate::plugins::preferences::snapshot(),
        ))
        .ok();
        if let Some(value) = key.as_ref().and_then(|key| cache.get(key)) {
            let _ = on_partial.send(value.clone());
            return Ok::<_, String>(value);
        }
        let result = crate::plugins::providers::info::file_info_snapshot_with_partial(
            resource,
            user_action,
            |sections| {
                let _ = on_partial.send(sections);
            },
        );
        if let Some(key) = key.filter(|_| !result.iter().any(|section| section.id.ends_with(".error"))) {
            cache.insert(key, result.clone());
        }
        Ok(result)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn file_info_directory_size(
    path: String,
    request_id: String,
) -> Result<u64, String> {
    if request_id.is_empty() || request_id.len() > 128 {
        return Err("invalid folder-size request id".into());
    }
    if crate::core::fs::provider_registry::resource_access(&path).map_err(String::from)?
        != crate::core::fs::provider_registry::ResourceAccess::HostPath
    {
        return Err("folder size calculation is available for local files only".into());
    }
    crate::plugins::resource::file_capabilities::require(
        &path,
        crate::plugins::provider_registry::FsOperation::List,
    )?;

    let cancelled = Arc::new(AtomicBool::new(false));
    {
        let mut requests = directory_size_requests()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if requests.contains_key(&request_id) {
            return Err("folder-size request id is already active".into());
        }
        requests.insert(request_id.clone(), (path.clone(), cancelled.clone()));
    }
    let worker_cancelled = cancelled.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        crate::plugins::providers::info::calculate_directory_size(
            std::path::Path::new(&path),
            &worker_cancelled,
        )
    })
    .await;
    {
        let mut requests = directory_size_requests()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if requests
            .get(&request_id)
            .is_some_and(|(_, current)| Arc::ptr_eq(current, &cancelled))
        {
            requests.remove(&request_id);
        }
    }
    result.map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) fn file_info_directory_size_cancel(path: String, request_id: String) {
    let cancelled = directory_size_requests()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get(&request_id)
        .filter(|(current_path, _)| current_path == &path)
        .map(|(_, cancelled)| Arc::clone(cancelled));
    if let Some(cancelled) = cancelled {
        cancelled.store(true, Ordering::Release);
    }
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
