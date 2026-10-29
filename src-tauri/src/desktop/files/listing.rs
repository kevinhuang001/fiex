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
