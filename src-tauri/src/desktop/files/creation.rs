use super::*;
use std::path::PathBuf;

/// Undo history only stores real host paths. Virtual adapters have their own
/// navigation lifetime and must never be replayed as local filesystem actions.
fn undoable(path: &str) -> bool {
    !crate::plugins::mounts::handles(path)
        && crate::usbfs::route(path).is_none()
        && !crate::core::volumes::virtual_resolvers::opaque(path)
}

#[tauri::command]
pub(crate) async fn create_entry(
    path: String,
    name: String,
    state: tauri::State<'_, Operations>,
) -> Result<String, String> {
    let created = tauri::async_runtime::spawn_blocking(move || {
        crate::core::fs::create(&path, &name, true, "core:create")
    })
    .await
    .map_err(|error| error.to_string())??;
    if undoable(&created) {
        state.record_created(PathBuf::from(&created));
    }
    Ok(created)
}

#[tauri::command]
pub(crate) async fn create_file_entry(path: String, name: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::core::fs::create(&path, &name, false, "core:create")
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn rename_entry(
    path: String,
    name: String,
    state: tauri::State<'_, Operations>,
) -> Result<String, String> {
    let source = path.clone();
    let renamed = tauri::async_runtime::spawn_blocking(move || {
        crate::core::fs::rename_path(&path, &name, "core:rename")
    })
    .await
    .map_err(|error| error.to_string())??;
    if undoable(&renamed) {
        state.record_rename(PathBuf::from(source), PathBuf::from(&renamed));
    }
    Ok(renamed)
}
