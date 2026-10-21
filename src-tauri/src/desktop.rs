use crate::{
    archives,
    drag_session::{DragSessions, DropPlan},
    files, git, git_history, git_remote,
    operations::{self, OperationResult, Operations},
    recent, remote,
    search::{SearchBatch, SearchState},
    tags, terminals,
    transfers::Resolution,
    volumes,
};
use notify::Watcher;
use std::{collections::HashMap, sync::Mutex};
use tauri::{ipc::Channel, Emitter, Manager};
type Watch = Mutex<HashMap<String, notify::RecommendedWatcher>>;
#[derive(Default)]
struct WindowPaths(Mutex<HashMap<String, String>>);
#[tauri::command]
fn ci_log(message: String) {
    #[cfg(feature = "ci-driver")]
    if std::env::var_os("FIEX_DRAG_DIAGNOSTICS").is_some() {
        eprintln!("ci: {message}");
    }
    #[cfg(not(feature = "ci-driver"))]
    let _ = message;
}
#[tauri::command]
fn ci_reply(app: tauri::AppHandle, id: u64, value: String) {
    #[cfg(feature = "ci-driver")]
    crate::ci_driver::reply(&app, id, value);
    #[cfg(not(feature = "ci-driver"))]
    let _ = (app, id, value);
}
#[tauri::command]
async fn register_drag(
    window: tauri::Window,
    app: tauri::AppHandle,
    paths: Vec<String>,
    copy_requested: bool,
) -> Result<Vec<String>, String> {
    if std::env::var_os("FIEX_DRAG_DIAGNOSTICS").is_some() {
        eprintln!("drag: register {:?} copy={copy_requested}", paths);
    }
    let mut moving = !copy_requested && !paths.iter().any(|p| remote::is_remote(p));
    for path in &paths {
        if remote::is_remote(path) {
            if std::path::Path::new(path).is_dir() {
                remote::download_dir(path).await?;
            } else {
                remote::ensure_downloaded(path).await?;
            }
        }
    }
    let label = window.label().to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let mut exported = Vec::new();
        for path in &paths {
            if let Some(route) = crate::usbfs::route(path) {
                let entry = crate::usbfs::entry(&route)?;
                if entry.is_dir {
                    return Err("虚拟卷文件夹请使用复制和粘贴".into());
                }
                moving &= !entry.readonly;
                exported.push(crate::usbfs::open_cached(&route)?);
            } else {
                exported.push(path.clone());
            }
        }
        app.state::<DragSessions>()
            .begin(&label, &paths, &exported, moving)?;
        if std::env::var_os("FIEX_DRAG_DIAGNOSTICS").is_some() {
            eprintln!("drag: session for {label} moving={moving} exported={exported:?}");
        }
        Ok(exported)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn finish_drag(window: tauri::Window, state: tauri::State<DragSessions>) {
    state.end(window.label());
}
#[tauri::command]
async fn internal_drop(
    app: tauri::AppHandle,
    paths: Vec<String>,
    consume: bool,
) -> Result<Option<DropPlan>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<DragSessions>().resolve(&paths, consume)
    })
    .await
    .map_err(|e| e.to_string())
}
#[tauri::command]
fn initial_path(window: tauri::Window, paths: tauri::State<WindowPaths>) -> Option<String> {
    let explicit = paths
        .0
        .lock()
        .unwrap()
        .get(window.label())
        .cloned()
        .or_else(|| {
            std::env::args()
                .nth(1)
                .filter(|p| std::path::Path::new(p).is_dir())
        });
    // Resolving the start folder here lets the first directory listing begin
    // immediately instead of waiting for the location survey.
    explicit.or_else(|| {
        dirs::document_dir()
            .filter(|path| path.is_dir())
            .or_else(|| dirs::home_dir().filter(|path| path.is_dir()))
            .map(|path| path.to_string_lossy().into_owned())
    })
}
#[tauri::command]
async fn new_window(app: tauri::AppHandle, path: String) -> Result<(), String> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(1);
    let mut config = app.config().app.windows[0].clone();
    config.label = format!("explorer-{}", NEXT.fetch_add(1, Ordering::Relaxed));
    config.title = format!(
        "Fiex · {}",
        std::path::Path::new(&path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
    );
    app.state::<WindowPaths>()
        .0
        .lock()
        .unwrap()
        .insert(config.label.clone(), path);
    let window = tauri::WebviewWindowBuilder::from_config(&app, &config)
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    crate::macos_material::configure(&window);
    let _ = window;
    Ok(())
}
#[tauri::command]
async fn git_remote_status(root: String) -> Result<git_remote::RemoteStatus, String> {
    tauri::async_runtime::spawn_blocking(move || git_remote::status(&root))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn git_remote_remove(root: String, name: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || git_remote::remove(&root, &name))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn git_remote_save(root: String, name: String, url: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || git_remote::save(&root, &name, &url))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn git_sync(
    root: String,
    remote: String,
    push: bool,
    branch: Option<String>,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        git_remote::sync(&root, &remote, push, branch.as_deref())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn drag_files(
    window: tauri::Window,
    paths: Vec<String>,
    on_event: Channel<String>,
) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        crate::native_drag::start(window, paths, on_event).await
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (window, paths, on_event);
        Err("此命令仅用于 GTK 文件拖动".into())
    }
}
#[tauri::command]
async fn git_init(path: String) -> Result<git::GitStatus, String> {
    tauri::async_runtime::spawn_blocking(move || git::init(&path))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn git_stage(root: String, paths: Vec<String>, all: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || git::stage(&root, &paths, all))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn git_unstage(root: String, paths: Vec<String>, all: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || git::unstage(&root, &paths, all))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn git_identity(root: String) -> Result<git::GitIdentity, String> {
    tauri::async_runtime::spawn_blocking(move || git::identity(&root))
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
async fn git_commit(
    root: String,
    message: String,
    name: String,
    email: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || git::commit(&root, &message, &name, &email))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn git_refs(root: String) -> Result<git::GitRefs, String> {
    tauri::async_runtime::spawn_blocking(move || git::refs(&root))
        .await
        .map_err(|error| error.to_string())?
}
#[tauri::command]
async fn git_history(root: String, skip: usize) -> Result<Vec<git_history::Commit>, String> {
    tauri::async_runtime::spawn_blocking(move || git_history::history(&root, skip))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn git_browse(root: String, reference: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || git_history::browse(&root, &reference))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn git_history_asset(path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || git_history::asset(&path))
        .await
        .map_err(|e| e.to_string())?
}
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
async fn locations() -> Result<Vec<volumes::Location>, String> {
    // Disk enumeration touches every mounted volume; keep it off the main
    // thread so it never delays the first paint or the first listing.
    tauri::async_runtime::spawn_blocking(volumes::locations)
        .await
        .map_err(|error| error.to_string())
}
#[tauri::command]
async fn list_directory(
    path: String,
    hidden: bool,
    ignores: Vec<String>,
    recent: tauri::State<'_, recent::Recent>,
) -> Result<files::Listing, String> {
    if path == "recent://" {
        return Ok(files::Listing {
            read_only: true,
            virtual_volume: true,
            path,
            entries: recent::entries(&recent),
            warnings: vec![],
        });
    }
    if path.starts_with("history://") {
        return tauri::async_runtime::spawn_blocking(move || {
            git_history::list(&path, hidden, &ignores)
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
            Ok(files::Listing {
                read_only: matches!(
                    route.kind,
                    crate::usbfs::FsKind::Apfs
                        | crate::usbfs::FsKind::Hfs
                        | crate::usbfs::FsKind::Btrfs
                ),
                virtual_volume: true,
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
        listing.read_only = remote::is_remote(&path);
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
fn relaunch_elevated(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let exe = std::env::current_exe().map_err(|error| error.to_string())?;
        let script = format!(
            "Start-Process -FilePath '{}' -Verb RunAs",
            exe.to_string_lossy().replace('\'', "''")
        );
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .creation_flags(0x00000010)
            .spawn()
            .map_err(|error| error.to_string())?;
        app.exit(0);
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        Err("当前系统不需要管理员重启".into())
    }
}
#[tauri::command]
async fn eject_usb(device: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || crate::usbfs::eject(&device))
        .await
        .map_err(|error| error.to_string())?
}
#[tauri::command]
async fn mount_device(device: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || volumes::mount_device(&device))
        .await
        .map_err(|error| error.to_string())?
}
#[tauri::command]
fn file_info(path: String) -> Result<files::Entry, String> {
    if path.starts_with("history://") {
        return git_history::info(&path);
    }
    if let Some(route) = crate::usbfs::route(&path) {
        return crate::usbfs::entry(&route);
    }
    files::entry(std::path::Path::new(&path))
}
#[tauri::command]
async fn read_preview(path: String) -> Result<String, String> {
    if path.starts_with("history://") {
        return tauri::async_runtime::spawn_blocking(move || git_history::preview(&path))
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
    resolutions: Option<Vec<Resolution>>,
) -> Result<OperationResult, String> {
    if remote::is_remote(&destination) {
        return Err("远程连接只读".into());
    }
    for source in &paths {
        if remote::is_remote(source) {
            if moving {
                return Err("远程连接只读，请复制文件".into());
            }
            if std::path::Path::new(source).is_dir() {
                remote::download_dir(source).await?;
            } else {
                remote::ensure_downloaded(source).await?;
            }
        }
    }
    if crate::usbfs::route(&destination).is_some()
        || paths.iter().any(|p| crate::usbfs::route(p).is_some())
    {
        let events = app.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            crate::virtual_transfers::transfer(
                &paths,
                &destination,
                moving,
                keep_both,
                &ignores,
                &resolutions.unwrap_or_default(),
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
    let result = tauri::async_runtime::spawn_blocking(move || {
        app.state::<Operations>().transfer_resolved(
            &paths,
            &destination,
            moving,
            keep_both,
            &ignores,
            &resolutions.unwrap_or_default(),
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
async fn recycle(app: tauri::AppHandle, paths: Vec<String>) -> Result<OperationResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut result = OperationResult {
            paths: vec![],
            errors: vec![],
            skipped: 0,
            conflicts: vec![],
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
    if root.starts_with("history://") {
        return tauri::async_runtime::spawn_blocking(move || {
            let started = std::time::Instant::now();
            let entries = git_history::search(&root, &query, hidden, &ignores)?;
            let _ = on_batch.send(SearchBatch {
                truncated: entries.len() >= 500,
                entries,
                done: true,
                elapsed_ms: started.elapsed().as_millis(),
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
async fn file_checksum(path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        use sha2::{Digest, Sha256};
        use std::io::Read;
        let mut file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let read = file.read(&mut buffer).map_err(|e| e.to_string())?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        Ok::<String, String>(
            hasher
                .finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
        )
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn pick_folder(default: Option<String>) -> Option<String> {
    let mut dialog = rfd::FileDialog::new();
    if let Some(directory) = default.filter(|path| std::path::Path::new(path).is_dir()) {
        dialog = dialog.set_directory(directory);
    }
    dialog
        .pick_folder()
        .map(|path| path.to_string_lossy().into_owned())
}
#[tauri::command]
async fn extract_archive(
    app: tauri::AppHandle,
    path: String,
    destination: Option<String>,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = archives::extract(&path, destination.as_deref())?;
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
    level: Option<u32>,
    password: Option<String>,
    ignores: Vec<String>,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = archives::compress(
            &paths,
            &destination,
            &format,
            &ignores,
            level,
            password.as_deref(),
        )?;
        app.state::<Operations>()
            .record_created(std::path::PathBuf::from(&result));
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn detect_terminals() -> Result<Vec<terminals::Terminal>, String> {
    tauri::async_runtime::spawn_blocking(terminals::detect)
        .await
        .map_err(|error| error.to_string())
}
#[tauri::command]
async fn detect_ides() -> Result<Vec<terminals::Ide>, String> {
    tauri::async_runtime::spawn_blocking(terminals::detect_ides)
        .await
        .map_err(|error| error.to_string())
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
async fn open_path(path: String, recent: tauri::State<'_, recent::Recent>) -> Result<(), String> {
    if crate::usbfs::route(&path).is_none() {
        recent::record(&recent, &path);
    }
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
async fn trash_path() -> Option<String> {
    tauri::async_runtime::spawn_blocking(|| {
        volumes::trash_path().map(|path| path.to_string_lossy().into_owned())
    })
    .await
    .ok()
    .flatten()
}
#[tauri::command]
fn open_applications() -> Result<(), String> {
    terminals::open_applications()
}
#[tauri::command]
fn set_tags(paths: Vec<String>, tags: Vec<tags::Tag>) -> Result<(), String> {
    tags::set_tags(&paths, &tags)
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
async fn empty_trash() -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(volumes::empty_trash)
        .await
        .map_err(|error| error.to_string())?
}
#[tauri::command]
async fn restore_from_trash(paths: Vec<String>) -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(move || volumes::restore_from_trash(&paths))
        .await
        .map_err(|error| error.to_string())?
}
#[tauri::command]
async fn delete_permanently(paths: Vec<String>) -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(move || volumes::delete_permanently(&paths))
        .await
        .map_err(|error| error.to_string())?
}
#[tauri::command]
fn watch_directory(
    window: tauri::Window,
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
    state
        .lock()
        .unwrap()
        .insert(window.label().to_string(), watcher);
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
        .manage(Watch::default())
        .manage(WindowPaths::default())
        .manage(recent::Recent::default())
        .manage(DragSessions::default())
        .invoke_handler(tauri::generate_handler![
            platform,
            ci_reply,
            ci_log,
            register_drag,
            finish_drag,
            internal_drop,
            initial_path,
            new_window,
            git_remote_status,
            git_remote_save,
            git_remote_remove,
            git_sync,
            drag_files,
            git_init,
            git_stage,
            git_unstage,
            git_commit,
            git_identity,
            git_status,
            git_refs,
            git_history,
            git_browse,
            git_history_asset,
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
            pick_folder,
            file_checksum,
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
            relaunch_elevated,
            mount_device,
            reveal_path,
            trash_path,
            empty_trash,
            restore_from_trash,
            delete_permanently,
            open_applications,
            set_tags,
            open_full_disk_access,
            open_trash,
            watch_directory,
            native_icon
        ])
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                window
                    .state::<Watch>()
                    .lock()
                    .unwrap()
                    .remove(window.label());
                window
                    .state::<WindowPaths>()
                    .0
                    .lock()
                    .unwrap()
                    .remove(window.label());
                window.state::<DragSessions>().end(window.label());
            }
        })
        .setup(|app| {
            #[cfg(feature = "ci-driver")]
            crate::ci_driver::start(app.handle().clone()).map_err(std::io::Error::other)?;
            #[cfg(target_os = "macos")]
            if let Some(window) = app.get_webview_window("main") {
                crate::macos_material::configure(&window);
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
