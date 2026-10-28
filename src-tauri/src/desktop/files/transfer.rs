use super::*;

// Tauri commands receive one argument per JS field, so the list is wide.
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
    // Mount sources are materialized into the local engine. The engine copies
    // them, and once every copy succeeds the provider-owned originals are
    // removed here, so a drag that means "move" really deletes from the VFS.
    let requested_moving = moving;
    let mut mounted_source = false;
    let mut originals = Vec::with_capacity(paths.len());
    let transfer_options = remote::TransferOptions {
        cancel: Some(cancel.flag()),
        progress: None,
    };
    let mut materialized = Vec::with_capacity(paths.len());
    for source in &paths {
        let is_mount = remote::is_remote(source) || crate::plugins::vfs::handles(source);
        if requested_moving && is_mount {
            crate::plugins::resource::file_capabilities::require(
                source,
                crate::plugins::provider_registry::FsOperation::Remove,
            )?;
        }
        originals.push(source.clone());
        if remote::is_remote(source) {
            mounted_source = true;
            if remote::stat(source)?.is_dir {
                remote::download_dir(source, &transfer_options).await?;
            } else {
                remote::ensure_downloaded(source, &transfer_options).await?;
            }
            materialized.push(remote::cached_path(source)?);
        } else if crate::plugins::vfs::handles(source) {
            mounted_source = true;
            materialized.push(crate::plugins::vfs::materialize(source)?);
        } else {
            crate::plugins::mounts::reject_local(source)?;
            materialized.push(source.clone());
        }
    }
    let destination_remote = remote::is_remote(&destination);
    if !destination_remote {
        crate::plugins::mounts::reject_local(&destination)?;
    }
    let paths = materialized;
    if destination_remote {
        crate::plugins::resource::file_capabilities::require(
            &destination,
            crate::plugins::provider_registry::FsOperation::Write,
        )?;
        if paths.iter().any(|path| std::path::Path::new(path).is_dir()) {
            crate::plugins::resource::file_capabilities::require(
                &destination,
                crate::plugins::provider_registry::FsOperation::Mkdir,
            )?;
        }
        let sources: Vec<remote::UploadSource> = paths
            .iter()
            .filter_map(|path| {
                let local = std::path::Path::new(path);
                let name = local.file_name()?.to_string_lossy().into_owned();
                Some(remote::UploadSource {
                    local: local.to_path_buf(),
                    name,
                })
            })
            .collect();
        let events = app.clone();
        let cancel = cancel.inner().clone();
        let emit = progress_emitter(on_progress);
        let progress = move |done: u64, total: u64, step: Option<&str>| emit(done, total, step);
        let resolutions = resolutions.clone().unwrap_or_default();
        let mut result = tauri::async_runtime::spawn_blocking(move || {
            remote::upload(
                &destination,
                &sources,
                keep_both,
                &ignores,
                &resolutions,
                &progress,
                &cancel.flag(),
            )
        })
        .await
        .map_err(|error| error.to_string())?;
        if result.as_ref().is_ok_and(|out| !out.paths.is_empty()) {
            let _ = events.emit("directory-changed", ());
        }
        if requested_moving {
            if let Ok(out) = &mut result {
                if out.errors.is_empty() && out.conflicts.is_empty() && out.skipped == 0 {
                    for original in &originals {
                        if let Err(error) = remove_moved_source(original).await {
                            out.errors.push(format!("{original}: {error}"));
                        }
                    }
                    let _ = events.emit("directory-changed", ());
                }
            }
        }
        return result;
    }
    // The engine only deletes sources it owns. Provider sources are removed
    // after a fully successful copy because their bytes live in the plugin.
    let engine_moving = requested_moving && !mounted_source;
    let events = app.clone();
    let mut result = if crate::usbfs::route(&destination).is_some()
        || paths.iter().any(|p| crate::usbfs::route(p).is_some())
    {
        let cancel = cancel.inner().clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            let flag = cancel.flag();
            crate::usbfs::transfers::transfer(
                &paths,
                &destination,
                engine_moving,
                keep_both,
                &ignores,
                &resolutions.unwrap_or_default(),
                &flag,
            )
        })
        .await
        .map_err(|e| e.to_string())?;
        if result.as_ref().is_ok_and(|r| !r.paths.is_empty()) {
            let _ = events.emit("directory-changed", ());
        }
        result
    } else {
        let cancel = cancel.inner().clone();
        let emit = progress_emitter(on_progress);
        let result = tauri::async_runtime::spawn_blocking(move || {
            let flag = cancel.flag();
            app.state::<Operations>().transfer_resolved_with_progress(
                &paths,
                &destination,
                engine_moving,
                keep_both,
                &ignores,
                &resolutions.unwrap_or_default(),
                &flag,
                &emit,
            )
        })
        .await
        .map_err(|e| e.to_string())?;
        if result.as_ref().is_ok_and(|r| !r.paths.is_empty()) {
            let _ = events.emit("directory-changed", ());
        }
        result
    };
    if requested_moving && mounted_source {
        if let Ok(out) = &mut result {
            // Do not remove originals after a skipped/conflicted/partial plan:
            // the copy did not cover every selected item.
            if out.errors.is_empty() && out.conflicts.is_empty() && out.skipped == 0 {
                for original in &originals {
                    if let Err(error) = remove_moved_source(original).await {
                        out.errors.push(format!("{original}: {error}"));
                    }
                }
                let _ = events.emit("directory-changed", ());
            }
        }
    }
    result
}
/// Delete a source that was only materialized for the engine. Provider paths
/// go back through their own adapter so caches and provider bytes both go away.
pub(super) async fn remove_moved_source(path: &str) -> Result<(), String> {
    if remote::is_remote(path) {
        return remote::remove(path).await;
    }
    if crate::plugins::vfs::handles(path) {
        let owned = path.to_string();
        return tauri::async_runtime::spawn_blocking(move || crate::plugins::vfs::remove(&owned))
            .await
            .map_err(|error| error.to_string())?;
    }
    let path = std::path::Path::new(path);
    if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
    .map_err(|error| error.to_string())
}
