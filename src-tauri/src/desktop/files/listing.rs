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
        let resource = crate::core::fs::trusted(&path, "core:preview")?;
        let bytes = resource.preview_prefix(512 * 1024)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Cache lookup never enumerates a provider or opens file contents.
#[tauri::command]
pub(crate) async fn directory_cache(
    path: String,
    hidden: bool,
    ignores: Vec<String>,
) -> Result<Option<files::Listing>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::core::fs::directory_cache::read(&path, hidden, &ignores)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DirectoryFrame {
    pub listing: files::Listing,
    pub decorations: Vec<crate::plugins::resolvers::decorations::Decoration>,
    pub done: bool,
    pub warning: Option<String>,
}
/// A single logical navigation request: emit an authorized complete cached
/// presentation immediately, then asynchronously revalidate the real provider.
#[tauri::command]
pub(crate) async fn directory_open(
    path: String,
    hidden: bool,
    ignores: Vec<String>,
    on_snapshot: tauri::ipc::Channel<DirectoryFrame>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        use crate::core::fs::directory_cache;
        use crate::plugins::provider_registry::FsOperation;
        use crate::plugins::resolvers::decorations::{self, DecorationEntry};
        let resource = crate::core::fs::trusted(&path, "core:list")?;
        resource.require(FsOperation::List, false)?;
        let cached = directory_cache::read(&path, hidden, &ignores)?;
        let before = if let Some(listing) = cached.as_ref() {
            let decoration = directory_cache::presentations(&path)?;
            let mut decorations = listing
                .entries
                .iter()
                .filter_map(|entry| decoration.get(&entry.path).cloned())
                .collect::<Vec<_>>();
            decorations.sort_by(|a, b| a.path.cmp(&b.path));
            let _ = on_snapshot.send(DirectoryFrame {
                listing: listing.clone(),
                decorations,
                done: false,
                warning: None,
            });
            Some(listing.clone())
        } else {
            None
        };
        let fresh = match resource.desktop_listing(hidden, &ignores) {
            Ok(listing) => listing,
            Err(error) => {
                if let Some(listing) = before {
                    let decoration = directory_cache::presentations(&path)?;
                    let _ = on_snapshot.send(DirectoryFrame {
                        decorations: listing
                            .entries
                            .iter()
                            .filter_map(|entry| decoration.get(&entry.path).cloned())
                            .collect(),
                        listing,
                        done: true,
                        warning: Some(error.to_string()),
                    });
                    return Ok(());
                }
                return Err(error.to_string());
            }
        };
        let candidates: Vec<DecorationEntry> = fresh
            .entries
            .iter()
            .filter_map(|entry| {
                serde_json::to_value(entry)
                    .ok()
                    .and_then(|value| serde_json::from_value(value).ok())
            })
            .collect();
        let decorations = decorations::query_with_plugins(&path, candidates, true);
        directory_cache::store_decorations(&path, &fresh.entries, decorations.clone());
        let _ = on_snapshot.send(DirectoryFrame {
            listing: fresh,
            decorations,
            done: true,
            warning: None,
        });
        Ok(())
    })
    .await
    .map_err(|error| error.to_string())?
}
