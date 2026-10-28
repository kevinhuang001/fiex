//! Platform, window, launch, terminal/IDE, tag, clipboard-adjacent and
//! CI-driver commands plus the per-window start path state.

use crate::{core::recent, core::tags, core::terminals};
use std::{collections::HashMap, sync::Mutex};
use tauri::Manager;

#[derive(Default)]
pub(crate) struct WindowPaths(pub(crate) Mutex<HashMap<String, String>>);
#[tauri::command]
pub(crate) fn ci_log(message: String) {
    #[cfg(feature = "ci-driver")]
    if std::env::var_os("FIEX_DRAG_DIAGNOSTICS").is_some() {
        eprintln!("ci: {message}");
    }
    #[cfg(not(feature = "ci-driver"))]
    let _ = message;
}
#[tauri::command]
pub(crate) fn ci_reply(app: tauri::AppHandle, id: u64, value: String) {
    #[cfg(feature = "ci-driver")]
    crate::ci_driver::reply(&app, id, value);
    #[cfg(not(feature = "ci-driver"))]
    let _ = (app, id, value);
}
#[tauri::command]
pub(crate) fn launch_path() -> Option<String> {
    std::env::args()
        .nth(1)
        .filter(|path| std::path::Path::new(path).is_dir())
}
#[tauri::command]
pub(crate) fn initial_path(
    window: tauri::Window,
    paths: tauri::State<WindowPaths>,
) -> Option<String> {
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
pub(crate) async fn new_window(app: tauri::AppHandle, path: String) -> Result<(), String> {
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
    crate::platform::macos_material::configure(&window);
    let _ = window;
    Ok(())
}
#[tauri::command]
pub(crate) fn system_proxy_info() -> crate::platform::system_proxy::ProxyInfo {
    crate::platform::system_proxy::info()
}
#[tauri::command]
pub(crate) fn platform() -> &'static str {
    std::env::consts::OS
}
#[tauri::command]
pub(crate) fn pick_folder(default: Option<String>) -> Option<String> {
    let mut dialog = rfd::FileDialog::new();
    if let Some(directory) = default.filter(|path| std::path::Path::new(path).is_dir()) {
        dialog = dialog.set_directory(directory);
    }
    dialog
        .pick_folder()
        .map(|path| path.to_string_lossy().into_owned())
}
#[tauri::command]
pub(crate) async fn detect_terminals() -> Result<Vec<terminals::Terminal>, String> {
    tauri::async_runtime::spawn_blocking(terminals::detect)
        .await
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub(crate) async fn detect_ides() -> Result<Vec<terminals::Ide>, String> {
    tauri::async_runtime::spawn_blocking(terminals::detect_ides)
        .await
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub(crate) async fn open_in_ide(path: String, id: String) -> Result<(), String> {
    crate::plugins::mounts::reject_local(&path)?;
    tauri::async_runtime::spawn_blocking(move || terminals::launch_ide(&path, &id))
        .await
        .map_err(|error| error.to_string())?
}
#[tauri::command]
pub(crate) fn open_external_url(url: String) -> Result<(), String> {
    let parsed = reqwest::Url::parse(&url).map_err(|_| crate::i18n::t("无效的链接"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(crate::i18n::t("只允许打开 http/https 链接").into());
    }
    open::that(url).map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn open_terminal(path: String, id: String) -> Result<(), String> {
    crate::plugins::mounts::reject_local(&path)?;
    terminals::launch(&path, &id)
}
#[tauri::command]
pub(crate) async fn open_path(
    path: String,
    recent: tauri::State<'_, recent::Recent>,
) -> Result<(), String> {
    if let Some(provider) = crate::core::volumes::virtual_resolvers::resolver_for(&path) {
        if path.starts_with(crate::core::volumes::applications::URI) {
            return provider.open(&path);
        }
        if crate::core::volumes::recycle::is_virtual(&path) {
            return provider.open(&path);
        }
    }
    recent::record(&recent, &path);
    tauri::async_runtime::spawn_blocking(move || {
        let exported = crate::core::fs::export_for_external_drag(&[path])?;
        terminals::open_path(exported.first().ok_or("missing export")?)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub(crate) fn reveal_path(path: String) -> Result<(), String> {
    crate::plugins::mounts::reject_local(&path)?;
    terminals::reveal(&path)
}
#[tauri::command]
pub(crate) fn open_applications() -> Result<(), String> {
    terminals::open_applications()
}
#[tauri::command]
pub(crate) fn set_tags(paths: Vec<String>, tags: Vec<tags::Tag>) -> Result<(), String> {
    for path in &paths {
        crate::plugins::mounts::reject_local(path)?;
    }
    tags::set_tags(&paths, &tags)
}
#[tauri::command]
pub(crate) fn open_full_disk_access() -> Result<(), String> {
    terminals::open_full_disk_access()
}
#[tauri::command]
pub(crate) fn relaunch_elevated(app: tauri::AppHandle) -> Result<(), String> {
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
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|error| error.to_string())?;
        app.exit(0);
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        Err(crate::i18n::t("当前系统不需要管理员重启").into())
    }
}
// Synchronous Tauri commands execute on the main thread, as required by GTK's icon provider.
#[tauri::command]
pub(crate) fn native_icon(path: String, size: Option<u16>) -> Result<String, String> {
    use base64::Engine;
    // Ask for pixels, not CSS points: the UI requests a size already scaled by
    // the display pixel ratio so application logos stay sharp in icon view.
    let pixels = size.unwrap_or(128).clamp(16, 512);
    let icon = file_icon_provider::get_file_icon(path, pixels).map_err(|e| e.to_string())?;
    let image = image::RgbaImage::from_raw(icon.width, icon.height, icon.pixels)
        .ok_or(crate::i18n::t("无效图标"))?;
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(out.into_inner())
    ))
}
