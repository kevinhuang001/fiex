use super::*;
use std::sync::atomic::Ordering;

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
    tauri::async_runtime::spawn_blocking(move || delete_selection(&paths, &flag))
        .await
        .map_err(|error| error.to_string())?
}

/// Validate every selected resource through its Provider before removing any
/// of them. Each removal reauthorizes at execution time to close the window
/// between batch preflight and mutation.
fn delete_selection(
    paths: &[String],
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<usize, String> {
    let resources = paths
        .iter()
        .map(|path| {
            let resource = crate::core::fs::trusted(path, "core:delete")?;
            resource
                .require(
                    crate::plugins::provider_registry::FsOperation::Remove,
                    false,
                )
                .map_err(String::from)?;
            resource
                .review_remove(crate::plugin_sdk::filesystem::PreflightPhase::Execute)
                .map_err(String::from)?;
            Ok(resource)
        })
        .collect::<Result<Vec<_>, String>>()?;

    let mut removed = 0;
    for resource in resources {
        if cancel.load(Ordering::Relaxed) {
            return Err(crate::core::operations::CANCELLED.into());
        }
        resource.remove().map_err(String::from)?;
        removed += 1;
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::delete_selection;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn preflights_the_whole_selection_before_removing_any_resource() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("must-survive.txt");
        std::fs::write(&file, b"preserve until every target is approved").unwrap();
        let paths = vec![
            file.to_string_lossy().into_owned(),
            crate::core::volumes::applications::URI.into(),
        ];

        let result = delete_selection(&paths, &AtomicBool::new(false));

        assert!(result.is_err());
        assert!(file.exists(), "the valid first item must not be deleted");
    }

    #[test]
    fn empty_applications_folder_uses_provider_remove_end_to_end() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let folder = crate::core::volumes::applications::create_virtual_folder(
            crate::core::volumes::applications::URI,
            &format!("delete-test-{}-{unique}", std::process::id()),
        )
        .expect("create test folder");

        let removed = delete_selection(std::slice::from_ref(&folder.path), &AtomicBool::new(false))
            .expect("empty virtual folder should be removable through the desktop path");

        assert_eq!(removed, 1);
        assert!(crate::core::volumes::applications::validate_remove(&folder.path).is_err());
    }

    #[test]
    fn nonempty_applications_folder_returns_the_provider_error() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let parent = crate::core::volumes::applications::create_virtual_folder(
            crate::core::volumes::applications::URI,
            &format!("parent-{unique}"),
        )
        .expect("create parent folder");
        let child = crate::core::volumes::applications::create_virtual_folder(
            &parent.path,
            &format!("child-{unique}"),
        )
        .expect("create child folder");

        let error = delete_selection(std::slice::from_ref(&parent.path), &AtomicBool::new(false))
            .expect_err("nonempty folder must not be deleted");

        assert!(
            error.contains("Directory not empty"),
            "unexpected error: {error}"
        );
        delete_selection(&[child.path], &AtomicBool::new(false)).expect("remove child");
        delete_selection(&[parent.path], &AtomicBool::new(false)).expect("remove parent");
    }
}
