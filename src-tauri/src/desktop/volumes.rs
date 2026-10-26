//! Volume, usbfs, mount and drag-and-drop commands.

use crate::{
    drag_session::{DragSessions, DropPlan},
    remote, volumes,
};
use tauri::{ipc::Channel, Manager};

#[tauri::command]
pub(crate) async fn register_drag(
    window: tauri::Window,
    app: tauri::AppHandle,
    paths: Vec<String>,
    copy_requested: bool,
) -> Result<Vec<String>, String> {
    if std::env::var_os("FIEX_DRAG_DIAGNOSTICS").is_some() {
        eprintln!("drag: register {:?} copy={copy_requested}", paths);
    }
    let mut moving = !copy_requested
        && !paths
            .iter()
            .any(|p| remote::is_remote(p) || crate::plugin_vfs::handles(p));
    let mut materialized = Vec::new();
    for path in &paths {
        if remote::is_remote(path) {
            if remote::stat(path)?.is_dir {
                remote::download_dir(path, &remote::TransferOptions::default()).await?;
            } else {
                remote::ensure_downloaded(path, &remote::TransferOptions::default()).await?;
            }
            materialized.push(remote::cached_path(path)?);
        } else if crate::plugin_vfs::handles(path) {
            materialized.push(crate::plugin_vfs::materialize(path)?);
        } else {
            crate::mounts::reject_local(path)?;
            materialized.push(path.clone());
        }
    }
    let original_paths = paths;
    let paths = materialized;
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
            .begin(&label, &original_paths, &exported, moving)?;
        if std::env::var_os("FIEX_DRAG_DIAGNOSTICS").is_some() {
            eprintln!("drag: session for {label} moving={moving} exported={exported:?}");
        }
        Ok(exported)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub(crate) fn finish_drag(window: tauri::Window, state: tauri::State<DragSessions>) {
    state.end(window.label());
}
#[tauri::command]
pub(crate) async fn internal_drop(
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
pub(crate) async fn drag_files(
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
pub(crate) fn eject_device(path: String) -> Result<(), String> {
    remote::eject(&path)
}
#[tauri::command]
pub(crate) async fn eject_usb(device: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || crate::usbfs::eject(&device))
        .await
        .map_err(|error| error.to_string())?
}
#[tauri::command]
pub(crate) async fn mount_device(device: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || volumes::mount_device(&device))
        .await
        .map_err(|error| error.to_string())?
}
