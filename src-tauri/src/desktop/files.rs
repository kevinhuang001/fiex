//! File system commands: directory listing, entry metadata and previews,
//! creation, renaming, transfers, permanent deletion and trash handling,
//! directory watching, and search.

use crate::{
    files,
    operations::{self, OperationCancel, OperationResult, Operations},
    recent, remote,
    search::{SearchBatch, SearchState},
    terminals,
    transfers::Resolution,
    volumes,
};
use notify::Watcher;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Mutex};
use tauri::{ipc::Channel, Emitter, Manager};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OperationProgress {
    done: u64,
    total: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    step: Option<String>,
    /// `bytes` renders sizes in the activity centre instead of file counts.
    #[serde(skip_serializing_if = "Option::is_none")]
    unit: Option<String>,
}

pub(crate) fn progress_emitter(
    channel: Channel<OperationProgress>,
) -> impl Fn(u64, u64, Option<&str>) + Send + Sync + 'static {
    move |done, total, step| {
        let _ = channel.send(OperationProgress {
            done,
            total,
            step: step.map(str::to_string),
            unit: None,
        });
    }
}

pub(crate) type ByteProgress = std::sync::Arc<dyn Fn(u64, Option<u64>, Option<&str>) + Send + Sync>;

/// Download progress is byte-based; the activity centre formats it as sizes.
pub(crate) fn byte_progress_emitter(channel: Channel<OperationProgress>) -> ByteProgress {
    std::sync::Arc::new(move |done, total, step| {
        let _ = channel.send(OperationProgress {
            done,
            total: total.unwrap_or(0),
            step: step.map(str::to_string),
            unit: Some("bytes".into()),
        });
    })
}
pub(crate) type Watch = Mutex<HashMap<String, (String, notify::RecommendedWatcher)>>;
#[tauri::command]
pub(crate) async fn list_directory(
    path: String,
    hidden: bool,
    ignores: Vec<String>,
    recent: tauri::State<'_, recent::Recent>,
) -> Result<files::Listing, String> {
    if path == "recent://" {
        return Ok(files::Listing {
            read_only: true,
            can_create: false,
            virtual_volume: true,
            path,
            entries: recent::entries(&recent),
            warnings: vec![],
        });
    }
    if remote::is_remote(&path) {
        return remote::list_listing(&path, hidden, &ignores)
            .await?
            .ok_or_else(|| "MountNotFound: connection removed".into());
    }
    if crate::plugin_vfs::handles(&path) {
        return tauri::async_runtime::spawn_blocking(move || {
            crate::plugin_vfs::list(&path, hidden, &ignores)
        })
        .await
        .map_err(|e| e.to_string())?;
    }

    if let Some(route) = crate::usbfs::route(&path) {
        let listing_path = path.clone();
        return tauri::async_runtime::spawn_blocking(move || {
            let mut entries = crate::usbfs::listing(&route)?;
            let filter = crate::ignores::IgnoreSet::new(&ignores);
            entries.retain(|entry| {
                (hidden || !entry.name.starts_with('.'))
                    && !filter.is_ignored(std::path::Path::new(&entry.name))
            });
            let read_only = matches!(
                route.kind,
                crate::usbfs::FsKind::Apfs
                    | crate::usbfs::FsKind::Hfs
                    | crate::usbfs::FsKind::Btrfs
            );
            Ok(files::Listing {
                read_only,
                can_create: !read_only,
                virtual_volume: true,
                path: listing_path,
                entries,
                warnings: vec![],
            })
        })
        .await
        .map_err(|error| error.to_string())?;
    }
    tauri::async_runtime::spawn_blocking(move || files::list(&path, hidden, &ignores))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub(crate) async fn file_info(path: String) -> Result<files::Entry, String> {
    tauri::async_runtime::spawn_blocking(move || file_info_blocking(&path))
        .await
        .map_err(|error| error.to_string())?
}

fn file_info_blocking(path: &str) -> Result<files::Entry, String> {
    if remote::is_remote(path) {
        return remote::stat(path);
    }
    if crate::plugin_vfs::handles(path) {
        return crate::plugin_vfs::stat(path);
    }
    if let Some(route) = crate::usbfs::route(path) {
        return crate::usbfs::entry(&route);
    }
    files::entry(std::path::Path::new(path))
}
#[tauri::command]
pub(crate) async fn read_preview(path: String) -> Result<String, String> {
    if remote::is_remote(&path) {
        let path = remote::ensure_downloaded(&path, &remote::TransferOptions::default()).await?;
        return tauri::async_runtime::spawn_blocking(move || files::text_preview(&path))
            .await
            .map_err(|e| e.to_string())?;
    }
    if crate::plugin_vfs::handles(&path) {
        return tauri::async_runtime::spawn_blocking(move || {
            let bytes = crate::plugin_vfs::read_prefix(&path, 512 * 1024)?;
            Ok(String::from_utf8_lossy(&bytes).into_owned())
        })
        .await
        .map_err(|e| e.to_string())?;
    }
    if let Some(route) = crate::usbfs::route(&path) {
        return tauri::async_runtime::spawn_blocking(move || {
            let bytes = crate::usbfs::read_path(&route)?;
            Ok(String::from_utf8_lossy(&bytes[..bytes.len().min(512 * 1024)]).into_owned())
        })
        .await
        .map_err(|e| e.to_string())?;
    }
    tauri::async_runtime::spawn_blocking(move || files::text_preview(&path))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub(crate) async fn create_entry(
    path: String,
    name: String,
    folder: bool,
    state: tauri::State<'_, Operations>,
) -> Result<String, String> {
    if !crate::plugin_vfs::handles(&path) {
        crate::mounts::reject_local(&path)?;
    }
    // A stale menu or toolbar must not create inside a provider that never
    // declared `mkdir`/`write` or a node that narrowed them away.
    crate::plugin_file_capabilities::require(
        &path,
        if folder {
            crate::plugin_providers::FsOperation::Mkdir
        } else {
            crate::plugin_providers::FsOperation::Write
        },
    )?;
    if let Some(route) = crate::usbfs::route(&path) {
        crate::usbfs::create(&route, &name, folder)?;
        return Ok(format!("{}/{}", path.trim_end_matches('/'), name));
    }
    if remote::is_remote(&path) {
        return remote::create(&path, &name, folder).await;
    }
    if crate::plugin_vfs::handles(&path) {
        return tauri::async_runtime::spawn_blocking(move || {
            crate::plugin_vfs::create(&path, &name, folder)
        })
        .await
        .map_err(|e| e.to_string())?;
    }
    state.create(&path, &name, folder)
}
#[tauri::command]
pub(crate) async fn rename_entry(
    path: String,
    name: String,
    state: tauri::State<'_, Operations>,
) -> Result<String, String> {
    if !crate::plugin_vfs::handles(&path) {
        crate::mounts::reject_local(&path)?;
    }
    crate::plugin_file_capabilities::require(&path, crate::plugin_providers::FsOperation::Rename)?;
    if let Some(route) = crate::usbfs::route(&path) {
        crate::usbfs::rename(&route, &name)?;
        return Ok(format!(
            "{}/{}",
            path.rsplit_once('/')
                .map(|(parent, _)| parent)
                .unwrap_or("/"),
            name
        ));
    }
    if remote::is_remote(&path) {
        return remote::rename(&path, &name).await;
    }
    if crate::plugin_vfs::handles(&path) {
        return tauri::async_runtime::spawn_blocking(move || {
            crate::plugin_vfs::rename(&path, &name)
        })
        .await
        .map_err(|e| e.to_string())?;
    }
    state.rename(&path, &name)
}
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
    if remote::is_remote(&destination) {
        return Err(crate::i18n::t("远程连接只读").into());
    }
    let transfer_options = remote::TransferOptions {
        cancel: Some(cancel.flag()),
        progress: None,
    };
    let mut materialized = Vec::with_capacity(paths.len());
    for source in &paths {
        if remote::is_remote(source) {
            if moving {
                return Err(crate::i18n::t("远程连接只读，请复制文件").into());
            }
            if remote::stat(source)?.is_dir {
                remote::download_dir(source, &transfer_options).await?;
            } else {
                remote::ensure_downloaded(source, &transfer_options).await?;
            }
            materialized.push(remote::cached_path(source)?);
        } else if crate::plugin_vfs::handles(source) {
            if moving {
                return Err("VFS view is read-only".into());
            }
            materialized.push(crate::plugin_vfs::materialize(source)?);
        } else {
            crate::mounts::reject_local(source)?;
            materialized.push(source.clone());
        }
    }
    crate::mounts::reject_local(&destination)?;
    let paths = materialized;
    if crate::usbfs::route(&destination).is_some()
        || paths.iter().any(|p| crate::usbfs::route(p).is_some())
    {
        let events = app.clone();
        let cancel = cancel.inner().clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            let flag = cancel.flag();
            crate::virtual_transfers::transfer(
                &paths,
                &destination,
                moving,
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
        return result;
    }
    let events = app.clone();
    let cancel = cancel.inner().clone();
    let emit = progress_emitter(on_progress);
    let result = tauri::async_runtime::spawn_blocking(move || {
        let flag = cancel.flag();
        app.state::<Operations>().transfer_resolved_with_progress(
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
    .map_err(|e| e.to_string())?;
    if result.as_ref().is_ok_and(|r| !r.paths.is_empty()) {
        let _ = events.emit("directory-changed", ());
    }
    result
}
#[tauri::command]
pub(crate) async fn recycle(
    app: tauri::AppHandle,
    paths: Vec<String>,
    on_progress: Channel<OperationProgress>,
    cancel: tauri::State<'_, OperationCancel>,
) -> Result<OperationResult, String> {
    for path in &paths {
        if !crate::plugin_vfs::handles(path) {
            crate::mounts::reject_local(path)?;
        }
        crate::plugin_file_capabilities::require(
            path,
            crate::plugin_providers::FsOperation::Remove,
        )?;
    }
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
            if remote::is_remote(&path) {
                match futures::executor::block_on(remote::remove(&path)) {
                    Ok(()) => result.paths.push(path),
                    Err(error) => result.errors.push(format!("{path}: {error}")),
                }
            } else if let Some(route) = crate::usbfs::route(&path) {
                match crate::usbfs::remove(&route) {
                    Ok(()) => result.paths.push(path),
                    Err(error) => result.errors.push(format!("{path}: {error}")),
                }
            } else if crate::plugin_vfs::handles(&path) {
                match crate::plugin_vfs::remove(&path) {
                    Ok(()) => result.paths.push(path),
                    Err(error) => result.errors.push(format!("{path}: {error}")),
                }
            } else {
                match operations::recycle(std::slice::from_ref(&path)) {
                    Ok(mut single) => {
                        result.paths.append(&mut single.paths);
                        result.errors.append(&mut single.errors);
                    }
                    Err(error) => result.errors.push(format!("{path}: {error}")),
                }
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
#[derive(Deserialize)]
pub(crate) struct SearchInput {
    pane: String,
    root: String,
    query: String,
    hidden: bool,
    ignores: Vec<String>,
}

#[tauri::command]
pub(crate) async fn search_files(
    window: tauri::Window,
    request: SearchInput,
    on_batch: Channel<SearchBatch>,
    state: tauri::State<'_, SearchState>,
) -> Result<(), String> {
    let SearchInput {
        pane,
        root,
        query,
        hidden,
        ignores,
    } = request;
    if remote::is_remote(&root) {
        let started = std::time::Instant::now();
        let mut pending = vec![root.clone()];
        let mut entries = Vec::new();
        let mut visited = 0;
        while let Some(path) = pending.pop() {
            visited += 1;
            if visited > 1000 || entries.len() >= 500 {
                break;
            }
            if let Some(listing) = remote::list_listing(&path, hidden, &ignores).await? {
                for entry in listing.entries {
                    if entry.is_dir {
                        pending.push(entry.path.clone());
                    }
                    if entry.name.to_lowercase().contains(&query.to_lowercase()) {
                        entries.push(entry);
                    }
                    if entries.len() >= 500 {
                        break;
                    }
                }
            }
        }
        let entries = crate::plugin_search::resolve(
            &crate::plugin_search::PluginSearchInput {
                query: query.clone(),
                root: root.clone(),
                hidden,
                ignores: ignores.clone(),
                limit: Some(500),
            },
            || Ok(serde_json::json!({"entries":entries})),
            || false,
        );
        let _ = on_batch.send(SearchBatch {
            entries,
            done: true,
            elapsed_ms: started.elapsed().as_millis(),
            truncated: !pending.is_empty() || visited > 1000,
            skipped: 0,
        });
        return Ok(());
    }
    if crate::plugin_vfs::handles(&root) {
        return tauri::async_runtime::spawn_blocking(move || {
            let started = std::time::Instant::now();
            let (entries, truncated) =
                crate::plugin_vfs::search(&root, &query, hidden, &ignores, 500)?;
            let entries = crate::plugin_search::resolve(
                &crate::plugin_search::PluginSearchInput {
                    query: query.clone(),
                    root: root.clone(),
                    hidden,
                    ignores: ignores.clone(),
                    limit: Some(500),
                },
                || Ok(serde_json::json!({"entries":entries})),
                || false,
            );
            let _ = on_batch.send(SearchBatch {
                entries,
                done: true,
                elapsed_ms: started.elapsed().as_millis(),
                truncated,
                skipped: 0,
            });
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())?;
    }
    if let Some(route) = crate::usbfs::route(&root) {
        return tauri::async_runtime::spawn_blocking(move || {
            let started = std::time::Instant::now();
            let entries = crate::usbfs::search(&route, &query, 500)?;
            let entries = crate::plugin_search::resolve(
                &crate::plugin_search::PluginSearchInput {
                    query: query.clone(),
                    root: root.clone(),
                    hidden,
                    ignores: ignores.clone(),
                    limit: Some(500),
                },
                || Ok(serde_json::json!({"entries":entries})),
                || false,
            );
            let _ = on_batch.send(SearchBatch {
                entries,
                done: true,
                elapsed_ms: started.elapsed().as_millis(),
                truncated: false,
                skipped: 0,
            });
            Ok(())
        })
        .await
        .map_err(|error| error.to_string())?;
    }
    let state = state.inner().clone();
    let scope = format!("{}:{pane}", window.label());
    tauri::async_runtime::spawn_blocking(move || {
        if root == "everywhere://" {
            state.run_global(&scope, &query, hidden, &ignores, |b| {
                let _ = on_batch.send(b);
            })
        } else {
            state.run(&scope, &root, &query, hidden, &ignores, |b| {
                let _ = on_batch.send(b);
            })
        }
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(())
}
#[tauri::command]
pub(crate) fn cancel_search(
    window: tauri::Window,
    pane: String,
    state: tauri::State<'_, SearchState>,
) {
    state.cancel(&format!("{}:{pane}", window.label()));
}
#[tauri::command]
pub(crate) fn begin_operation(state: tauri::State<'_, OperationCancel>) {
    state.begin();
}
#[tauri::command]
pub(crate) fn cancel_operation(state: tauri::State<'_, OperationCancel>) {
    state.cancel();
}
#[tauri::command]
pub(crate) fn watch_directory(
    window: tauri::Window,
    app: tauri::AppHandle,
    path: String,
    pane: String,
    state: tauri::State<'_, Watch>,
) -> Result<(), String> {
    crate::mounts::reject_local(&path)?;
    let mut watcher =
        notify::recommended_watcher(move |event: Result<notify::Event, notify::Error>| {
            if let Ok(event) = event {
                if !matches!(event.kind, notify::EventKind::Access(_)) {
                    let _ = app.emit("directory-changed", ());
                }
            }
        })
        .map_err(|e| e.to_string())?;
    watcher
        .watch(
            std::path::Path::new(&path),
            notify::RecursiveMode::NonRecursive,
        )
        .map_err(|e| e.to_string())?;
    state
        .lock()
        .unwrap()
        .insert(format!("{}:{pane}", window.label()), (path, watcher));
    Ok(())
}
#[tauri::command]
pub(crate) fn unwatch_directory(
    window: tauri::Window,
    path: String,
    pane: String,
    state: tauri::State<'_, Watch>,
) {
    let mut watches = state.lock().unwrap();
    let key = format!("{}:{pane}", window.label());
    if watches
        .get(&key)
        .is_some_and(|(current, _)| current == &path)
    {
        watches.remove(&key);
    }
}
#[tauri::command]
pub(crate) async fn trash_path() -> Option<String> {
    tauri::async_runtime::spawn_blocking(|| {
        volumes::trash_path().map(|path| path.to_string_lossy().into_owned())
    })
    .await
    .ok()
    .flatten()
}
#[tauri::command]
pub(crate) fn open_trash() -> Result<(), String> {
    terminals::open_trash()
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
        crate::mounts::reject_local(path)?;
    }
    if let Some(path) = &fallback {
        crate::mounts::reject_local(path)?;
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
    for path in &paths {
        if !crate::plugin_vfs::handles(path) {
            crate::mounts::reject_local(path)?;
        }
        crate::plugin_file_capabilities::require(
            path,
            crate::plugin_providers::FsOperation::Remove,
        )?;
    }
    let flag = cancel.flag();
    tauri::async_runtime::spawn_blocking(move || {
        let mut removed = 0;
        let mut local = Vec::new();
        for path in paths {
            if remote::is_remote(&path) {
                futures::executor::block_on(remote::remove(&path))?;
                removed += 1;
            } else if crate::plugin_vfs::handles(&path) {
                crate::plugin_vfs::remove(&path)?;
                removed += 1;
            } else {
                local.push(path);
            }
        }
        removed += volumes::delete_permanently(&local, &flag)?;
        Ok(removed)
    })
    .await
    .map_err(|error| error.to_string())?
}
