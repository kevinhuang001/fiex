//! Remote connection commands and the volume locations survey.

use super::files::{byte_progress_emitter, progress_emitter, OperationProgress};
use crate::{
    core::operations::OperationCancel, core::recent, core::terminals, core::volumes, remote,
};
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
pub(crate) async fn connection_open(id: String) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || remote::open_navigation(&id))
        .await
        .map_err(|error| error.to_string())?
}
#[tauri::command]
pub(crate) fn remote_connection_config(id: String) -> Result<remote::ConnectionInput, String> {
    remote::connection_config(&id)
}
#[tauri::command]
pub(crate) async fn save_remote_connection(
    input: remote::ConnectionInput,
) -> Result<remote::ConnectionInfo, String> {
    // Saving validates the provider and may open the browser for OAuth. Keep
    // that wait off the main thread so the window can paint its loading state.
    tauri::async_runtime::spawn_blocking(move || remote::save_connection(input))
        .await
        .map_err(|error| error.to_string())?
}
#[tauri::command]
pub(crate) fn update_connection_session(
    id: String,
    saved: Option<bool>,
    autostart: Option<bool>,
) -> Result<remote::ConnectionInfo, String> {
    remote::update_connection_session(&id, saved, autostart)
}

#[tauri::command]
pub(crate) fn remove_remote_connection(id: String) -> Result<(), String> {
    remote::remove_connection(&id)
}
#[tauri::command]
pub(crate) async fn remote_download(
    path: String,
    on_progress: Channel<OperationProgress>,
    cancel: tauri::State<'_, OperationCancel>,
) -> Result<String, String> {
    let emit = byte_progress_emitter(on_progress);
    let step = std::path::Path::new(&path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string);
    let options = remote::TransferOptions {
        cancel: Some(cancel.flag()),
        progress: Some(std::sync::Arc::new(move |done, total| {
            emit(done, total, step.as_deref())
        })),
    };
    remote::download(&path, &options).await
}
#[tauri::command]
pub(crate) async fn remote_cache(
    paths: Vec<String>,
    on_progress: Channel<OperationProgress>,
    cancel: tauri::State<'_, OperationCancel>,
) -> Result<(), String> {
    let emit = byte_progress_emitter(on_progress);
    let cancel = cancel.flag();
    for path in &paths {
        let entry = remote::stat(path)?;
        let step = std::path::Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string);
        let emit = emit.clone();
        let options = remote::TransferOptions {
            cancel: Some(cancel.clone()),
            progress: Some(std::sync::Arc::new(move |done, total| {
                emit(done, total, step.as_deref())
            })),
        };
        if entry.is_dir {
            remote::download_dir(path, &options).await?;
        } else {
            remote::download(path, &options).await?;
        }
    }
    Ok(())
}
/// Opening a remote file downloads it first; report that download as progress.
#[tauri::command]
pub(crate) async fn remote_open(
    path: String,
    on_progress: Channel<OperationProgress>,
    recent: tauri::State<'_, recent::Recent>,
) -> Result<(), String> {
    let emit = byte_progress_emitter(on_progress);
    let step = std::path::Path::new(&path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string);
    let options = remote::TransferOptions {
        cancel: None,
        progress: Some(std::sync::Arc::new(move |done, total| {
            emit(done, total, step.as_deref())
        })),
    };
    recent::record(&recent, &path);
    let local = remote::ensure_downloaded(&path, &options).await?;
    tauri::async_runtime::spawn_blocking(move || terminals::open_path(&local))
        .await
        .map_err(|error| error.to_string())?
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
pub(crate) async fn remote_uncache(paths: Vec<String>) -> Result<(), String> {
    remote::uncache(&paths).await
}

#[tauri::command]
pub(crate) async fn remote_unpin(paths: Vec<String>) -> Result<(), String> {
    remote::unpin(&paths).await
}

#[tauri::command]
pub(crate) fn remote_set_sync_interval(seconds: u64) {
    remote::set_sync_interval(seconds);
}

#[tauri::command]
pub(crate) async fn remote_resolve(path: String, keep_local: bool) -> Result<(), String> {
    remote::resolve(&path, keep_local).await
}
