use super::*;

// A desktop selection is bound to Core-owned ResourceRefs at invocation start.
// Every source and destination then uses the same broker and bounded stream.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub(crate) async fn transfer(
    app: tauri::AppHandle,
    paths: Vec<String>,
    destination: String,
    moving: bool,
    keep_both: bool,
    ignores: Vec<String>,
    resolutions: Option<Vec<Resolution>>,
    on_progress: Channel<OperationProgress>,
    cancel: tauri::State<'_, OperationCancel>,
) -> Result<OperationResult, String> {
    // Keep ResourceRefs in the owning storage adapter. The transfer engine
    // itself checks source read and destination child/create capabilities.
    // Materialized OS paths must never become the transfer destination.
    let flag = cancel.flag();
    let emit = progress_emitter(on_progress);
    let result = tauri::async_runtime::spawn_blocking(move || {
        crate::core::fs::transfer(
            &paths,
            &destination,
            moving,
            keep_both,
            &ignores,
            &resolutions.unwrap_or_default(),
            &flag,
            &emit,
        )
    })
    .await
    .map_err(|error| error.to_string())??;
    if !result.paths.is_empty() {
        let _ = app.emit("directory-changed", ());
    }
    Ok(result)
}
