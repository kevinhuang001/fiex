use crate::{
    drag_session::{DragSessions, DropPlan},
    files, git, git_history, git_remote,
    operations::{self, OperationCancel, OperationResult, Operations},
    recent, remote,
    search::{SearchBatch, SearchState},
    tags, terminals,
    transfers::Resolution,
    volumes,
};
use notify::Watcher;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};
use tauri::{
    ipc::Channel,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager,
};
static QUITTING: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OperationProgress {
    done: u64,
    total: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    step: Option<String>,
}

fn progress_emitter(
    channel: Channel<OperationProgress>,
) -> impl Fn(u64, u64, Option<&str>) + Send + Sync + 'static {
    move |done, total, step| {
        let _ = channel.send(OperationProgress {
            done,
            total,
            step: step.map(str::to_string),
        });
    }
}

fn show_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
/// Closing the window keeps Fiex alive in the tray; only the tray menu quits.
fn setup_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "显示 Fiex", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出 Fiex", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;
    let mut builder = TrayIconBuilder::with_id("fiex-tray")
        .tooltip("Fiex")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "quit" => {
                QUITTING.store(true, Ordering::SeqCst);
                app.exit(0);
            }
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}
type Watch = Mutex<HashMap<String, (String, notify::RecommendedWatcher)>>;
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
fn launch_path() -> Option<String> {
    std::env::args()
        .nth(1)
        .filter(|path| std::path::Path::new(path).is_dir())
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
/// Generic plugin process bridge. Deliberately runs on the blocking pool:
/// stdout/IPC must never block the WebView or Tauri's async scheduler.
#[tauri::command]
async fn plugin_invoke(
    plugin_id: String,
    method: String,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::plugin_host::global().invoke(&plugin_id, &method, params)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn plugin_prepare_install(
    repository: String,
) -> Result<crate::plugin_installer::InstallPreview, String> {
    tauri::async_runtime::spawn_blocking(move || crate::plugin_installer::prepare(&repository))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn plugin_confirm_install(ticket: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || crate::plugin_installer::install(&ticket))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
fn plugin_cancel_install(ticket: String) -> Result<(), String> {
    crate::plugin_installer::cancel(&ticket)
}
#[tauri::command]
async fn plugin_uninstall(plugin_id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || crate::plugin_installer::uninstall(&plugin_id))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
fn plugin_set_enabled(plugin_id: String, enabled: bool) -> Result<(), String> {
    crate::plugin_host::set_enabled(&plugin_id, enabled)
}
#[tauri::command]
fn plugin_set_permission(
    plugin_id: String,
    permission: String,
    granted: bool,
) -> Result<(), String> {
    crate::plugin_host::set_permission(&plugin_id, &permission, granted)
}
#[tauri::command]
fn plugin_approve_install(plugin_id: String) -> Result<(), String> {
    crate::plugin_host::approve_install(&plugin_id)
}
#[tauri::command]
fn plugin_status() -> Vec<(String, bool)> {
    crate::plugin_host::catalog()
        .into_iter()
        .map(|item| (item.id, item.backend_available))
        .collect()
}
#[tauri::command]
fn plugins_list() -> Vec<crate::plugin_host::PluginInfo> {
    crate::plugin_host::catalog()
}
fn ensure_plugin(id: &str) -> Result<(), String> {
    if crate::plugin_host::exists(id) {
        Ok(())
    } else {
        Err("plugin is not installed".into())
    }
}
#[tauri::command]
fn plugin_storage_get(
    plugin_id: String,
    scope: String,
    key: String,
) -> Result<Option<serde_json::Value>, String> {
    ensure_plugin(&plugin_id)?;
    crate::plugin_storage::get(&plugin_id, &scope, &key)
}
#[tauri::command]
fn plugin_storage_set(
    plugin_id: String,
    scope: String,
    key: String,
    value: serde_json::Value,
) -> Result<(), String> {
    ensure_plugin(&plugin_id)?;
    crate::plugin_storage::set(&plugin_id, &scope, &key, value)
}
#[tauri::command]
fn plugin_storage_delete(plugin_id: String, scope: String, key: String) -> Result<(), String> {
    ensure_plugin(&plugin_id)?;
    crate::plugin_storage::delete(&plugin_id, &scope, &key)
}
#[tauri::command]
fn plugin_secret_set(
    plugin_id: String,
    scope: String,
    key: String,
    value: String,
) -> Result<(), String> {
    ensure_plugin(&plugin_id)?;
    let permitted = crate::plugin_host::catalog()
        .into_iter()
        .find(|p| p.id == plugin_id)
        .and_then(|p| p.contributes.get("settings").cloned())
        .and_then(|v| v.as_array().cloned())
        .is_some_and(|fields| {
            fields.iter().any(|f| {
                f.get("id").and_then(|v| v.as_str()) == Some(key.as_str())
                    && f.get("type").and_then(|v| v.as_str()) == Some("secret")
            })
        });
    if !permitted || scope != "global" {
        return Err("secret setting is not declared".into());
    }
    crate::plugin_storage::secret_set(&plugin_id, &scope, &key, &value)
}
#[tauri::command]
fn plugin_secret_available() -> bool {
    crate::plugin_storage::secret_available()
}

/// Temporary v0.2.7 compatibility path: prefer external plugin whenever
/// installed. A plugin RPC error must not silently fall back to legacy code.
fn plugin_compat(
    id: &str,
    method: &str,
    params: serde_json::Value,
) -> Result<Option<serde_json::Value>, String> {
    let host = crate::plugin_host::global();
    if !host.is_installed(id) {
        return Ok(None);
    }
    host.invoke(id, method, params).map(Some)
}

#[tauri::command]
async fn git_remote_status(root: String) -> Result<git_remote::RemoteStatus, String> {
    tauri::async_runtime::spawn_blocking(move || git_remote::status(&root))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn git_remote_remove(root: String, name: String) -> Result<(), String> {
    if let Some(v) = tauri::async_runtime::spawn_blocking({
        let root = root.clone();
        let name = name.clone();
        move || {
            plugin_compat(
                "org.fiex.git",
                "git.remoteRemove",
                serde_json::json!({"root":root,"name":name}),
            )
        }
    })
    .await
    .map_err(|e| e.to_string())??
    {
        return serde_json::from_value(v).map_err(|e| e.to_string());
    }
    tauri::async_runtime::spawn_blocking(move || git_remote::remove(&root, &name))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn git_remote_save(root: String, name: String, url: String) -> Result<(), String> {
    if let Some(v) = tauri::async_runtime::spawn_blocking({
        let root = root.clone();
        let name = name.clone();
        let url = url.clone();
        move || {
            plugin_compat(
                "org.fiex.git",
                "git.remoteSave",
                serde_json::json!({"root":root,"name":name,"url":url}),
            )
        }
    })
    .await
    .map_err(|e| e.to_string())??
    {
        return serde_json::from_value(v).map_err(|e| e.to_string());
    }
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
    if let Some(v) = tauri::async_runtime::spawn_blocking({
        let root = root.clone();
        let remote = remote.clone();
        let branch = branch.clone();
        move || {
            plugin_compat(
                "org.fiex.git",
                "git.remoteSync",
                serde_json::json!({"root":root,"remote":remote,"push":push,"branch":branch}),
            )
        }
    })
    .await
    .map_err(|e| e.to_string())??
    {
        return serde_json::from_value(v).map_err(|e| e.to_string());
    }
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
    if let Some(v) = tauri::async_runtime::spawn_blocking({
        let root = root.clone();
        let paths = paths.clone();
        move || {
            plugin_compat(
                "org.fiex.git",
                "git.stage",
                serde_json::json!({"root":root,"paths":paths,"all":all}),
            )
        }
    })
    .await
    .map_err(|e| e.to_string())??
    {
        return serde_json::from_value(v).map_err(|e| e.to_string());
    }
    tauri::async_runtime::spawn_blocking(move || git::stage(&root, &paths, all))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn git_unstage(root: String, paths: Vec<String>, all: bool) -> Result<(), String> {
    if let Some(v) = tauri::async_runtime::spawn_blocking({
        let root = root.clone();
        let paths = paths.clone();
        move || {
            plugin_compat(
                "org.fiex.git",
                "git.unstage",
                serde_json::json!({"root":root,"paths":paths,"all":all}),
            )
        }
    })
    .await
    .map_err(|e| e.to_string())??
    {
        return serde_json::from_value(v).map_err(|e| e.to_string());
    }
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
    if let Some(v) = tauri::async_runtime::spawn_blocking({
        let root = root.clone();
        let message = message.clone();
        let name = name.clone();
        let email = email.clone();
        move || {
            plugin_compat(
                "org.fiex.git",
                "git.commit",
                serde_json::json!({"root":root,"message":message,"name":name,"email":email}),
            )
        }
    })
    .await
    .map_err(|e| e.to_string())??
    {
        return serde_json::from_value(v).map_err(|e| e.to_string());
    }
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
    if let Some(v) = tauri::async_runtime::spawn_blocking({
        let root = root.clone();
        let reference = reference.clone();
        move || {
            plugin_compat(
                "org.fiex.git",
                "git.browse",
                serde_json::json!({"root":root,"reference":reference}),
            )
        }
    })
    .await
    .map_err(|e| e.to_string())??
    {
        return serde_json::from_value(v).map_err(|e| e.to_string());
    }
    tauri::async_runtime::spawn_blocking(move || git_history::browse(&root, &reference))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn git_history_asset(path: String) -> Result<String, String> {
    if let Some(v) = tauri::async_runtime::spawn_blocking({
        let path = path.clone();
        move || {
            plugin_compat(
                "org.fiex.git",
                "git.historyAsset",
                serde_json::json!({"path":path}),
            )
        }
    })
    .await
    .map_err(|e| e.to_string())??
    {
        return serde_json::from_value(v).map_err(|e| e.to_string());
    }
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
async fn remote_sync(
    paths: Vec<String>,
    on_progress: Channel<OperationProgress>,
) -> Result<(), String> {
    let emit = progress_emitter(on_progress);
    let total = paths.len() as u64;
    for (index, path) in paths.iter().enumerate() {
        remote::sync_one(path).await?;
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
// Tauri commands receive one argument per JS field, so the list is wide.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
async fn transfer(
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
async fn recycle(
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
async fn undo(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<Operations>().undo())
        .await
        .map_err(|e| e.to_string())?
}
#[derive(Deserialize)]
struct SearchInput {
    pane: String,
    root: String,
    query: String,
    hidden: bool,
    ignores: Vec<String>,
}

#[tauri::command]
async fn search_files(
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
async fn warm_index(
    root: String,
    hidden: bool,
    ignores: Vec<String>,
    state: tauri::State<'_, SearchState>,
) -> Result<(), String> {
    if root == "everywhere://" || crate::usbfs::route(&root).is_some() {
        return Ok(());
    }
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.warm(&root, hidden, &ignores))
        .await
        .map_err(|e| e.to_string())??;
    Ok(())
}
#[tauri::command]
fn cancel_search(window: tauri::Window, pane: String, state: tauri::State<'_, SearchState>) {
    state.cancel(&format!("{}:{pane}", window.label()));
}
#[tauri::command]
fn begin_operation(state: tauri::State<'_, OperationCancel>) {
    state.begin();
}
#[tauri::command]
fn cancel_operation(state: tauri::State<'_, OperationCancel>) {
    state.cancel();
}
#[tauri::command]
async fn file_checksums(path: String) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::plugin_host::global().invoke(
            "org.fiex.checksum",
            "checksum.calculate",
            serde_json::json!({"path":path}),
        )
    })
    .await
    .map_err(|e| e.to_string())?
}
/// Host-owned native file chooser. Returns only a selected path; plugin
/// code never gets a handle to the chooser UI or a directory browsing grant.
#[tauri::command]
fn plugin_file_dialog(
    mode: String,
    directory: Option<String>,
    suggested_name: Option<String>,
    extensions: Option<Vec<String>>,
) -> Result<Option<String>,String> {
    let mut dialog = rfd::FileDialog::new();
    if let Some(path)=directory.filter(|v|std::path::Path::new(v).is_dir()){
        dialog=dialog.set_directory(path);
    }
    if let Some(name)=suggested_name {
        let path=std::path::Path::new(&name);
        if path.file_name().is_none() || path.components().count()!=1 || name.contains(['\\','/','\0']){
            return Err("invalid suggested filename".into());
        }
        dialog=dialog.set_file_name(name);
    }
    if let Some(exts)=extensions {
        if exts.len()>20 || exts.iter().any(|v|v.is_empty()||v.len()>16||
            !v.chars().all(|c|c.is_ascii_alphanumeric())) {
            return Err("invalid file extension filter".into());
        }
        if !exts.is_empty() {
            let refs=exts.iter().map(String::as_str).collect::<Vec<_>>();
            dialog=dialog.add_filter("文件类型",&refs);
        }
    }
    let selection=match mode.as_str(){
        "openFile"=>dialog.pick_file(),
        "openDirectory"=>dialog.pick_folder(),
        "saveFile"=>dialog.save_file(),
        _=>return Err("unsupported file picker mode".into())
    };
    Ok(selection.map(|p|p.to_string_lossy().into_owned()))
}
#[tauri::command]
fn plugin_files_dialog(directory:Option<String>,extensions:Option<Vec<String>>)
    ->Result<Option<Vec<String>>,String> {
    let mut dialog=rfd::FileDialog::new();
    if let Some(path)=directory.filter(|v|std::path::Path::new(v).is_dir()){
        dialog=dialog.set_directory(path);
    }
    if let Some(exts)=extensions {
        if exts.len()>20 || exts.iter().any(|v|v.is_empty()||v.len()>16||
            !v.chars().all(|c|c.is_ascii_alphanumeric())) {return Err("invalid extension filter".into());}
        if !exts.is_empty(){
            let refs=exts.iter().map(String::as_str).collect::<Vec<_>>();
            dialog=dialog.add_filter("文件类型",&refs);
        }
    }
    Ok(dialog.pick_files().map(|paths|paths.into_iter().map(|p|p.to_string_lossy().into_owned()).collect()))
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
        let result = crate::plugin_host::global().invoke(
            "org.fiex.archive",
            "archive.extract",
            serde_json::json!({"path":path,"destination":destination}),
        )?;
        let extracted: String = serde_json::from_value(result).map_err(|e| e.to_string())?;
        app.state::<Operations>()
            .record_created(std::path::PathBuf::from(&extracted));
        Ok(extracted)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[allow(clippy::too_many_arguments)]
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
        let response = crate::plugin_host::global().invoke(
            "org.fiex.archive",
            "archive.compress",
            serde_json::json!({"paths":paths,"destination":destination,"format":format,
                "level":level,"password":password,"ignores":ignores}),
        )?;
        let created: String = serde_json::from_value(response).map_err(|e| e.to_string())?;
        app.state::<Operations>()
            .record_created(std::path::PathBuf::from(&created));
        Ok(created)
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
async fn empty_trash(cancel: tauri::State<'_, OperationCancel>) -> Result<usize, String> {
    let flag = cancel.flag();
    tauri::async_runtime::spawn_blocking(move || volumes::empty_trash(&flag))
        .await
        .map_err(|error| error.to_string())?
}
#[tauri::command]
fn trash_restore_needs_folder(paths: Vec<String>) -> bool {
    volumes::restore_needs_folder(&paths)
}
#[tauri::command]
async fn restore_from_trash(
    paths: Vec<String>,
    fallback: Option<String>,
    cancel: tauri::State<'_, OperationCancel>,
) -> Result<usize, String> {
    let flag = cancel.flag();
    tauri::async_runtime::spawn_blocking(move || {
        volumes::restore_from_trash(&paths, fallback.as_deref(), &flag)
    })
    .await
    .map_err(|error| error.to_string())?
}
#[tauri::command]
async fn delete_permanently(
    paths: Vec<String>,
    cancel: tauri::State<'_, OperationCancel>,
) -> Result<usize, String> {
    let flag = cancel.flag();
    tauri::async_runtime::spawn_blocking(move || volumes::delete_permanently(&paths, &flag))
        .await
        .map_err(|error| error.to_string())?
}
#[tauri::command]
fn watch_directory(
    window: tauri::Window,
    app: tauri::AppHandle,
    path: String,
    pane: String,
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
        .insert(format!("{}:{pane}", window.label()), (path, watcher));
    Ok(())
}
#[tauri::command]
fn unwatch_directory(
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
        .manage(OperationCancel::default())
        .manage(SearchState::default())
        .manage(Watch::default())
        .manage(WindowPaths::default())
        .manage(recent::Recent::default())
        .manage(DragSessions::default())
        .invoke_handler(tauri::generate_handler![
            platform,
            plugin_invoke,
            plugin_status,
            plugins_list,
            plugin_prepare_install,
            plugin_confirm_install,
            plugin_cancel_install,
            plugin_uninstall,
            plugin_set_enabled,
            plugin_set_permission,
            plugin_approve_install,
            plugin_storage_get,
            plugin_storage_set,
            plugin_storage_delete,
            plugin_secret_set,
            plugin_secret_available,
            ci_reply,
            ci_log,
            register_drag,
            finish_drag,
            internal_drop,
            initial_path,
            launch_path,
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
            begin_operation,
            cancel_operation,
            extract_archive,
            pick_folder,
            plugin_file_dialog,
            plugin_files_dialog,
            file_checksums,
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
            trash_restore_needs_folder,
            delete_permanently,
            open_applications,
            set_tags,
            open_full_disk_access,
            open_trash,
            watch_directory,
            unwatch_directory,
            native_icon
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" && !QUITTING.load(Ordering::SeqCst) {
                    api.prevent_close();
                    let _ = window.hide();
                    return;
                }
            }
            if matches!(event, tauri::WindowEvent::Destroyed) {
                window
                    .state::<Watch>()
                    .lock()
                    .unwrap()
                    .retain(|key, _| !key.starts_with(&format!("{}:", window.label())));
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
            // Maintain per-volume Tantivy indices in the background throughout
            // this application session. Permission-denied paths are skipped.
            app.state::<SearchState>().inner().clone().start_global_daemon();
            #[cfg(feature = "ci-driver")]
            crate::ci_driver::start(app.handle().clone()).map_err(std::io::Error::other)?;
            #[cfg(target_os = "macos")]
            if let Some(window) = app.get_webview_window("main") {
                crate::macos_material::configure(&window);
            }
            setup_tray(app.handle())?;
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
