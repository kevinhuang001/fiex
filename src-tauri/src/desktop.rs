use crate::{
    archives, files, git,
    operations::{self, OperationResult, Operations},
    remote,
    search::{SearchBatch, SearchState},
    terminals,
};
use notify::Watcher;
use std::sync::Mutex;
use tauri::{ipc::Channel, Emitter, Manager};
type Watch = Mutex<Option<notify::RecommendedWatcher>>;
#[tauri::command]
async fn git_status(path: String) -> Option<git::GitStatus> {
    tauri::async_runtime::spawn_blocking(move || git::status(&path))
        .await
        .ok()
        .flatten()
}
#[tauri::command]
fn platform() -> &'static str {
    std::env::consts::OS
}
#[tauri::command]
fn locations() -> Vec<files::Location> {
    files::locations()
}
#[tauri::command]
async fn list_directory(
    path: String,
    hidden: bool,
    ignores: Vec<String>,
) -> Result<files::Listing, String> {
    if let Some(route) = crate::usbfs::route(&path) {
        let listing_path = path.clone();
        return tauri::async_runtime::spawn_blocking(move || {
            let entries = crate::usbfs::listing(&route)?;
            Ok(files::Listing {
                path: listing_path,
                entries,
                warnings: vec![],
            })
        })
        .await
        .map_err(|error| error.to_string())?;
    }
    let warning = remote::sync_dir(&path).await.err();
    tauri::async_runtime::spawn_blocking(move || {
        let mut listing = files::list(&path, hidden, &ignores)?;
        remote::apply_meta(&mut listing.entries);
        if let Some(warning) = warning {
            listing
                .entries
                .retain(|entry| entry.sync.as_deref() != Some("cloud"));
            listing.warnings.push(format!("远程刷新失败：{warning}"));
        }
        Ok(listing)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn remote_connections() -> Vec<remote::ConnectionInfo> {
    remote::list_connections()
}
#[tauri::command]
fn save_remote_connection(
    input: remote::ConnectionInput,
) -> Result<remote::ConnectionInfo, String> {
    remote::save_connection(input)
}
#[tauri::command]
fn remove_remote_connection(id: String) -> Result<(), String> {
    remote::remove_connection(&id)
}
#[tauri::command]
async fn remote_download(path: String) -> Result<String, String> {
    remote::download(&path).await
}
#[tauri::command]
async fn remote_download_dir(path: String) -> Result<(), String> {
    remote::download_dir(&path).await
}
#[tauri::command]
async fn remote_sync(paths: Vec<String>) -> Result<(), String> {
    remote::sync(&paths).await
}
#[tauri::command]
async fn remote_unsync(paths: Vec<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || remote::unsync(&paths))
        .await
        .map_err(|error| error.to_string())?
}
#[tauri::command]
fn eject_device(path: String) -> Result<(), String> {
    remote::eject(&path)
}
#[tauri::command]
async fn eject_usb(device: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || crate::usbfs::eject(&device))
        .await
        .map_err(|error| error.to_string())?
}
#[tauri::command]
async fn mount_device(device: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || files::mount_device(&device))
        .await
        .map_err(|error| error.to_string())?
}
#[tauri::command]
fn file_info(path: String) -> Result<files::Entry, String> {
    if let Some(route) = crate::usbfs::route(&path) {
        return crate::usbfs::entry(&route);
    }
    files::entry(std::path::Path::new(&path))
}
#[tauri::command]
async fn read_preview(path: String) -> Result<String, String> {
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
fn create_entry(
    path: String,
    name: String,
    folder: bool,
    state: tauri::State<'_, Operations>,
) -> Result<String, String> {
    if let Some(route) = crate::usbfs::route(&path) {
        crate::usbfs::create(&route, &name, folder)?;
        return Ok(format!("{}/{}", path.trim_end_matches('/'), name));
    }
    state.create(&path, &name, folder)
}
#[tauri::command]
fn rename_entry(
    path: String,
    name: String,
    state: tauri::State<'_, Operations>,
) -> Result<String, String> {
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
    state.rename(&path, &name)
}
#[tauri::command]
async fn transfer(
    app: tauri::AppHandle,
    paths: Vec<String>,
    destination: String,
    moving: bool,
    keep_both: bool,
    ignores: Vec<String>,
) -> Result<OperationResult, String> {
    if let Some(route) = crate::usbfs::route(&destination) {
        return tauri::async_runtime::spawn_blocking(move || {
            let mut result = OperationResult {
                paths: vec![],
                errors: vec![],
                skipped: 0,
            };
            for source in &paths {
                if crate::usbfs::route(source).is_some() {
                    result
                        .errors
                        .push(format!("{source}: 暂不支持从 U 盘虚拟卷复制"));
                    continue;
                }
                match crate::usbfs::copy_local_into(
                    &route,
                    std::path::Path::new(source),
                    keep_both,
                    moving,
                ) {
                    Ok(path) => result.paths.push(path),
                    Err(error) => result.errors.push(format!("{source}: {error}")),
                }
            }
            Ok(result)
        })
        .await
        .map_err(|e| e.to_string())?;
    }
    if paths.iter().any(|path| crate::usbfs::route(path).is_some()) {
        return tauri::async_runtime::spawn_blocking(move || {
            let mut result = OperationResult {
                paths: vec![],
                errors: vec![],
                skipped: 0,
            };
            for source in &paths {
                if let Some(route) = crate::usbfs::route(source) {
                    match crate::usbfs::copy_to_local(
                        &route,
                        std::path::Path::new(&destination),
                        keep_both,
                        moving,
                    ) {
                        Ok(path) => result.paths.push(path),
                        Err(error) => result.errors.push(format!("{source}: {error}")),
                    }
                } else {
                    match app.state::<Operations>().transfer(
                        std::slice::from_ref(source),
                        &destination,
                        moving,
                        keep_both,
                        &ignores,
                    ) {
                        Ok(mut single) => {
                            result.paths.append(&mut single.paths);
                            result.errors.append(&mut single.errors);
                            result.skipped += single.skipped;
                        }
                        Err(error) => result.errors.push(format!("{source}: {error}")),
                    }
                }
            }
            Ok(result)
        })
        .await
        .map_err(|e| e.to_string())?;
    }
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<Operations>()
            .transfer(&paths, &destination, moving, keep_both, &ignores)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn recycle(app: tauri::AppHandle, paths: Vec<String>) -> Result<OperationResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut result = OperationResult {
            paths: vec![],
            errors: vec![],
            skipped: 0,
        };
        for path in paths {
            if let Some(route) = crate::usbfs::route(&path) {
                match crate::usbfs::remove(&route) {
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
        if !result.paths.is_empty() {
            app.state::<Operations>().clear_undo();
        }
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn undo(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<Operations>().undo())
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn search_files(
    root: String,
    query: String,
    hidden: bool,
    ignores: Vec<String>,
    on_batch: Channel<SearchBatch>,
    state: tauri::State<'_, SearchState>,
) -> Result<(), String> {
    if let Some(route) = crate::usbfs::route(&root) {
        return tauri::async_runtime::spawn_blocking(move || {
            let started = std::time::Instant::now();
            let entries = crate::usbfs::search(&route, &query, 500)?;
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
    tauri::async_runtime::spawn_blocking(move || {
        state.run(&root, &query, hidden, &ignores, |b| {
            let _ = on_batch.send(b);
        })
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(())
}
#[tauri::command]
async fn warm_index(
    root: String,
    hidden: bool,
    ignores: Vec<String>,
    state: tauri::State<'_, SearchState>,
) -> Result<(), String> {
    if crate::usbfs::route(&root).is_some() {
        return Ok(());
    }
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.warm(&root, hidden, &ignores))
        .await
        .map_err(|e| e.to_string())??;
    Ok(())
}
#[tauri::command]
fn cancel_search(state: tauri::State<'_, SearchState>) {
    state.cancel();
}
#[tauri::command]
async fn extract_archive(app: tauri::AppHandle, path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = archives::extract(&path)?;
        app.state::<Operations>()
            .record_created(std::path::PathBuf::from(&result));
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn compress_archive(
    app: tauri::AppHandle,
    paths: Vec<String>,
    destination: String,
    format: String,
    ignores: Vec<String>,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = archives::compress(&paths, &destination, &format, &ignores)?;
        app.state::<Operations>()
            .record_created(std::path::PathBuf::from(&result));
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn detect_terminals() -> Vec<terminals::Terminal> {
    terminals::detect()
}
#[tauri::command]
fn detect_ides() -> Vec<terminals::Ide> {
    terminals::detect_ides()
}
#[tauri::command]
async fn open_in_ide(path: String, id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || terminals::launch_ide(&path, &id))
        .await
        .map_err(|error| error.to_string())?
}
#[tauri::command]
fn open_terminal(path: String, id: String) -> Result<(), String> {
    terminals::launch(&path, &id)
}
#[tauri::command]
async fn open_path(path: String) -> Result<(), String> {
    if let Some(route) = crate::usbfs::route(&path) {
        return tauri::async_runtime::spawn_blocking(move || {
            let bytes = crate::usbfs::read_path(&route)?;
            let cache = dirs::home_dir()
                .ok_or("无法创建缓存目录")?
                .join(".fiex")
                .join("usbfs-cache")
                .join(
                    std::path::Path::new(&route.root)
                        .file_name()
                        .unwrap_or_default(),
                )
                .join(&route.rel);
            if let Some(parent) = cache.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(&cache, bytes).map_err(|e| e.to_string())?;
            terminals::open_path(&cache.to_string_lossy())
        })
        .await
        .map_err(|e| e.to_string())?;
    }
    let path = remote::ensure_downloaded(&path).await?;
    tauri::async_runtime::spawn_blocking(move || terminals::open_path(&path))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
fn reveal_path(path: String) -> Result<(), String> {
    terminals::reveal(&path)
}
#[tauri::command]
fn trash_path() -> Option<String> {
    files::trash_path().map(|path| path.to_string_lossy().into_owned())
}
#[tauri::command]
fn open_applications() -> Result<(), String> {
    terminals::open_applications()
}
#[tauri::command]
fn set_tags(paths: Vec<String>, tags: Vec<files::Tag>) -> Result<(), String> {
    files::set_tags(&paths, &tags)
}
#[tauri::command]
fn open_full_disk_access() -> Result<(), String> {
    terminals::open_full_disk_access()
}
#[tauri::command]
fn open_trash() -> Result<(), String> {
    terminals::open_trash()
}
#[tauri::command]
fn watch_directory(
    app: tauri::AppHandle,
    path: String,
    state: tauri::State<'_, Watch>,
) -> Result<(), String> {
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
    *state.lock().unwrap() = Some(watcher);
    Ok(())
}
// Synchronous Tauri commands execute on the main thread, as required by GTK's icon provider.
#[tauri::command]
fn native_icon(path: String, size: Option<u16>) -> Result<String, String> {
    use base64::Engine;
    // Ask for pixels, not CSS points: the UI requests a size already scaled by
    // the display pixel ratio so application logos stay sharp in icon view.
    let pixels = size.unwrap_or(128).clamp(16, 512);
    let icon = file_icon_provider::get_file_icon(path, pixels).map_err(|e| e.to_string())?;
    let image =
        image::RgbaImage::from_raw(icon.width, icon.height, icon.pixels).ok_or("无效图标")?;
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(out.into_inner())
    ))
}
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_drag::init())
        .manage(Operations::default())
        .manage(SearchState::default())
        .manage(Mutex::new(None::<notify::RecommendedWatcher>))
        .invoke_handler(tauri::generate_handler![
            platform,
            git_status,
            locations,
            list_directory,
            file_info,
            read_preview,
            create_entry,
            rename_entry,
            transfer,
            recycle,
            undo,
            search_files,
            warm_index,
            cancel_search,
            extract_archive,
            compress_archive,
            detect_terminals,
            detect_ides,
            open_in_ide,
            open_terminal,
            open_path,
            remote_connections,
            save_remote_connection,
            remove_remote_connection,
            remote_download,
            remote_download_dir,
            remote_sync,
            remote_unsync,
            eject_device,
            eject_usb,
            mount_device,
            reveal_path,
            trash_path,
            open_applications,
            set_tags,
            open_full_disk_access,
            open_trash,
            watch_directory,
            native_icon
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            if let Some(window) = app.get_webview_window("main") {
                super::macos_material::configure(&window);
            }
            #[cfg(target_os = "windows")]
            if let Some(window) = app.get_webview_window("main") {
                use std::ffi::c_void;
                #[link(name = "dwmapi")]
                unsafe extern "system" {
                    fn DwmSetWindowAttribute(
                        hwnd: *mut c_void,
                        attribute: u32,
                        value: *const c_void,
                        size: u32,
                    ) -> i32;
                }
                if let Ok(hwnd) = window.hwnd() {
                    let rounded = 2_u32;
                    unsafe {
                        DwmSetWindowAttribute(hwnd.0, 33, &rounded as *const _ as *const c_void, 4);
                    }
                }
            }
            let _ = app;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Cannot start Fiex");
}
