use super::Shared;
use aria2_core::{DownloadEngine, DownloadStatus, request::request_group_man::RequestGroupMan};
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};

// A process-wide Tokio scheduler avoids creating two new I/O workers and
// initializing a fresh reactor for every download. Each download retains its
// own request group, cancellation handle and temporary output.
fn shared_runtime() -> Result<&'static tokio::runtime::Runtime, String> {
    static RUNTIME: std::sync::OnceLock<Result<tokio::runtime::Runtime, String>> =
        std::sync::OnceLock::new();
    RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .enable_all()
                .build()
                .map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(Clone::clone)
}

pub(super) fn run(
    url: String,
    options: aria2_core::request::request_group::DownloadOptions,
    payload: PathBuf,
    expected: Option<u64>,
    shared: Arc<Mutex<Shared>>,
    mut cancel: tokio::sync::watch::Receiver<bool>,
) {
    let result = match shared_runtime() {
        Ok(runtime) => runtime.block_on(async move {
            let groups = Arc::new(RequestGroupMan::new());
            let engine = match DownloadEngine::new().start_with_request_group_man(groups) {
                Ok(engine) => engine,
                Err(error) => return Err(error.to_string()),
            };
            let download = match engine.downloads().add_uri(vec![url], options) {
                Ok(download) => download,
                Err(error) => {
                    let _ = engine.shutdown_and_wait().await;
                    return Err(error.to_string());
                }
            };
            let outcome = tokio::select! {
                result = download.wait() => result.map_err(|error| error.to_string()),
                _ = async {
                    while cancel.changed().await.is_ok() {
                        if *cancel.borrow() {
                            break;
                        }
                    }
                } => {
                    let _ = download.remove();
                    Err("aria2 download cancelled".to_string())
                }
            };
            let _ = engine.shutdown_and_wait().await;
            outcome
        }),
        Err(error) => Err(error.to_string()),
    };
    if let Ok(mut shared) = shared.lock() {
        match result {
            Ok(result) if result.status == DownloadStatus::Complete => {
                shared.done = true;
                // A successful engine status is insufficient if the remote
                // object changed between metadata and download.
                match fs::metadata(&payload) {
                    Ok(meta) if expected.is_some_and(|size| size != meta.len()) => {
                        shared.error = Some(format!(
                            "aria2 download size mismatch: expected {} bytes, got {}",
                            expected.unwrap_or_default(),
                            meta.len()
                        ));
                    }
                    Ok(meta) => shared.total = meta.len(),
                    Err(error) => shared.error = Some(format!("aria2 output missing: {error}")),
                }
            }
            Ok(result) => {
                shared.done = true;
                shared.error = Some(if result.message.is_empty() {
                    format!("aria2 download ended with {:?}", result.status)
                } else {
                    result.message
                });
            }
            Err(error) => {
                shared.done = true;
                shared.error = Some(error);
            }
        }
    }
}
