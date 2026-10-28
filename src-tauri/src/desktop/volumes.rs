//! Volume, usbfs, mount and drag-and-drop commands.

use crate::{
    core::drag_session::{DragSessions, DropPlan},
    core::volumes,
    remote,
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
    // Internal move is a node capability, not an assumption about which
    // backend owns the resource. Recent/Trash and launch-only items fail
    // the remove check and remain copy-only.
    let moving = !copy_requested
        && paths
            .iter()
            .all(|path| crate::plugins::resource::file_capabilities::for_path(path).remove);
    let originals = paths.clone();
    let materialized = tauri::async_runtime::spawn_blocking(move || {
        crate::core::fs::export_for_external_drag(&paths)
    })
    .await
    .map_err(|e| e.to_string())??;
    let original_paths = originals;
    let paths = materialized;
    let label = window.label().to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let mut exported = Vec::new();
        exported.extend(paths.iter().cloned());
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
    captured: Option<DropPlan>,
) -> Result<Option<DropPlan>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let plan = app
            .state::<DragSessions>()
            .resolve(&paths, consume)
            .or(captured);
        if let Some(plan) = &plan {
            plan.validate()?;
        }
        Ok(plan)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub(crate) async fn drag_files(
    window: tauri::Window,
    paths: Vec<String>,
    on_event: Channel<String>,
) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        crate::platform::native_drag::start(window, paths, on_event).await
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (window, paths, on_event);
        Err(crate::i18n::t("此命令仅用于 GTK 文件拖动").into())
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
