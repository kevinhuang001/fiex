use super::*;

#[tauri::command]
pub(crate) async fn recycle(
    app: tauri::AppHandle,
    paths: Vec<String>,
    on_progress: Channel<OperationProgress>,
    cancel: tauri::State<'_, OperationCancel>,
) -> Result<OperationResult, String> {
    let cancel = cancel.inner().clone();
    let emit = progress_emitter(on_progress);
    let total = paths.len() as u64;
    tauri::async_runtime::spawn_blocking(move || {
        let mut result = OperationResult {
            paths: vec![],
            errors: vec![],
            skipped: 0,
            conflicts: vec![],
        };
        for (index, path) in paths.into_iter().enumerate() {
            if cancel.is_cancelled() {
                // Keep the undo history consistent with the items that did move.
                if !result.paths.is_empty() {
                    app.state::<Operations>().clear_undo();
                }
                return Err(operations::CANCELLED.into());
            }
            let step = std::path::Path::new(&path)
                .file_name()
                .and_then(|name| name.to_str());
            emit(index as u64 + 1, total, step);
            match crate::core::fs::move_to_trash(&path, "core:trash") {
                Ok(()) => result.paths.push(path),
                Err(error) => result.errors.push(format!("{path}: {error}")),
            }
        }
        if total > 0 {
            emit(total, total, None);
        }
        if !result.paths.is_empty() {
            app.state::<Operations>().clear_undo();
        }
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) async fn undo(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<Operations>().undo())
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) async fn trash_path() -> Option<String> {
    Some(volumes::trash_vfs::URI.into())
}

#[tauri::command]
pub(crate) async fn empty_trash(
    cancel: tauri::State<'_, OperationCancel>,
) -> Result<usize, String> {
    let flag = cancel.flag();
    tauri::async_runtime::spawn_blocking(move || volumes::empty_trash(&flag))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) fn trash_restore_needs_folder(paths: Vec<String>) -> bool {
    volumes::restore_needs_folder(&paths)
}

#[tauri::command]
pub(crate) async fn restore_from_trash(
    paths: Vec<String>,
    fallback: Option<String>,
    cancel: tauri::State<'_, OperationCancel>,
) -> Result<usize, String> {
    for path in &paths {
        crate::plugins::mounts::reject_local(path)?;
    }
    if let Some(path) = &fallback {
        crate::plugins::mounts::reject_local(path)?;
    }
    let flag = cancel.flag();
    tauri::async_runtime::spawn_blocking(move || {
        volumes::restore_from_trash(&paths, fallback.as_deref(), &flag)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) async fn delete_permanently(
    paths: Vec<String>,
    cancel: tauri::State<'_, OperationCancel>,
) -> Result<usize, String> {
    let flag = cancel.flag();
    tauri::async_runtime::spawn_blocking(move || {
        let mut removed = 0;
        let mut trash = Vec::new();
        for path in paths {
            if crate::core::volumes::virtual_resolvers::view_for(&path)
                .is_some_and(|view| view.id() == "core:trash")
            {
                trash.push(path);
                continue;
            }
            crate::core::fs::remove_path(&path, "core:delete")?;
            removed += 1;
        }
        removed += volumes::delete_permanently(&trash, &flag)?;
        Ok(removed)
    })
    .await
    .map_err(|error| error.to_string())?
}
