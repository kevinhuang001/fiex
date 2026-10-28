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
    let applications_destination =
        destination == volumes::applications::URI || volumes::applications::is_user(&destination);
    let destination = if applications_destination {
        volumes::applications::stored_path(&destination)?
            .to_string_lossy()
            .into_owned()
    } else {
        destination
    };
    let paths = paths
        .into_iter()
        .map(|path| {
            if volumes::applications::is_user(&path) {
                volumes::applications::stored_path(&path)
                    .map(|path| path.to_string_lossy().into_owned())
            } else if volumes::virtual_resolvers::opaque(&path) {
                Err("Unsupported: view entry has no transferable content".into())
            } else {
                Ok(path)
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    let flag = cancel.flag();
    let emit = progress_emitter(on_progress);
    let mut result = tauri::async_runtime::spawn_blocking(move || {
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
    if applications_destination {
        for path in &mut result.paths {
            *path = volumes::applications::stored_uri(std::path::Path::new(path))?;
        }
    }
    Ok(result)
}
