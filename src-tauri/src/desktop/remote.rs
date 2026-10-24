//! Remote connection commands and the volume locations survey.

use super::files::{progress_emitter, OperationProgress};
use crate::{operations::OperationCancel, remote, volumes};
use tauri::ipc::Channel;

#[tauri::command]
pub(crate) async fn locations() -> Result<Vec<volumes::Location>, String> {
    // Disk enumeration touches every mounted volume; keep it off the main
    // thread so it never delays the first paint or the first listing.
    tauri::async_runtime::spawn_blocking(volumes::locations)
        .await
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub(crate) fn remote_connections() -> Vec<remote::ConnectionInfo> {
    remote::list_connections()
}
#[tauri::command]
pub(crate) fn save_remote_connection(
    input: remote::ConnectionInput,
) -> Result<remote::ConnectionInfo, String> {
    remote::save_connection(input)
}
#[tauri::command]
pub(crate) fn remove_remote_connection(id: String) -> Result<(), String> {
    remote::remove_connection(&id)
}
#[tauri::command]
pub(crate) async fn remote_download(
    path: String,
    cancel: tauri::State<'_, OperationCancel>,
) -> Result<String, String> {
    let options = remote::TransferOptions {
        cancel: Some(cancel.flag()),
        progress: None,
    };
    remote::download(&path, &options).await
}
#[tauri::command]
pub(crate) async fn remote_download_dir(
    path: String,
    cancel: tauri::State<'_, OperationCancel>,
) -> Result<(), String> {
    let options = remote::TransferOptions {
        cancel: Some(cancel.flag()),
        progress: None,
    };
    remote::download_dir(&path, &options).await
}
#[tauri::command]
pub(crate) async fn remote_sync(
    paths: Vec<String>,
    on_progress: Channel<OperationProgress>,
    cancel: tauri::State<'_, OperationCancel>,
) -> Result<(), String> {
    let emit = progress_emitter(on_progress);
    let total = paths.len() as u64;
    let options = remote::TransferOptions {
        cancel: Some(cancel.flag()),
        progress: None,
    };
    for (index, path) in paths.iter().enumerate() {
        remote::sync_one(path, &options).await?;
        let step = std::path::Path::new(path)
            .file_name()
            .and_then(|name| name.to_str());
        emit(index as u64 + 1, total, step);
    }
    if total > 0 {
        emit(total, total, None);
    }
    Ok(())
}
#[tauri::command]
pub(crate) async fn remote_unsync(paths: Vec<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || remote::unsync(&paths))
        .await
        .map_err(|error| error.to_string())?
}
