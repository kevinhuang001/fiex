use super::*;

#[tauri::command]
pub(crate) async fn list_directory(
    path: String,
    hidden: bool,
    ignores: Vec<String>,
    recent: tauri::State<'_, recent::Recent>,
) -> Result<files::Listing, String> {
    let _ = recent; // Recent storage is read by its in-process adapter.
    tauri::async_runtime::spawn_blocking(move || {
        crate::core::fs::trusted(&path, "core:list")?
            .desktop_listing(hidden, &ignores)
            .map_err(String::from)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub(crate) async fn file_info(path: String) -> Result<files::Entry, String> {
    tauri::async_runtime::spawn_blocking(move || file_info_blocking(&path))
        .await
        .map_err(|error| error.to_string())?
}

pub(super) fn file_info_blocking(path: &str) -> Result<files::Entry, String> {
    crate::core::fs::trusted(path, "core:stat")?
        .desktop_entry()
        .map_err(String::from)
}

#[tauri::command]
pub(crate) async fn read_preview(path: String) -> Result<String, String> {
    // Keep the view identity bound to Core; the adapter resolves its bytes.
    tauri::async_runtime::spawn_blocking(move || {
        use crate::infrastructure::cache::memory::MemoryCache;
        use std::{sync::OnceLock, time::Duration};
        static TEXT: OnceLock<MemoryCache<String>> = OnceLock::new();
        let cache = TEXT.get_or_init(|| MemoryCache::new(32, Duration::from_secs(60)));
        let resource = crate::core::fs::trusted(&path, "core:preview")?;
        // A cached preview never replaces the provider's live read authorization.
        resource.require(crate::plugins::provider_registry::FsOperation::Read, false)?;
        // Remote mount paths can be rebound to a different account while
        // retaining identical size/mtime. Do not reuse preview bytes across
        // those identities; only local stable files are eligible.
        let key = if crate::plugins::mounts::handles(&path) {
            None
        } else {
            resource.desktop_entry().ok().and_then(|entry| {
                serde_json::to_string(&(path.as_str(), entry.modified, entry.size)).ok()
            })
        };
        if let Some(value) = key.as_ref().and_then(|key| cache.get(key)) {
            return Ok(value);
        }
        let bytes = resource.preview_prefix(512 * 1024)?;
        let text = String::from_utf8_lossy(&bytes).into_owned();
        if let Some(key) = key {
            cache.insert(key, text.clone());
        }
        Ok::<String, String>(text)
    })
    .await
    .map_err(|e| e.to_string())?
}
