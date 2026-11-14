use super::tray::setup_tray;
use tauri::Manager;

pub(super) fn configure(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    super::system::set_desktop_handle(app.handle().clone());
    super::scheduler::spawn(app.handle().clone());
    {
        use tauri::Emitter;
        let handle = app.handle().clone();
        crate::plugins::task_events::set_sink(Box::new(move |event| {
            let _ = handle.emit("plugin-task", &event);
        }));
        let handle = app.handle().clone();
        crate::plugins::security::prompt::set_sink(Box::new(move |event| match event {
            crate::plugins::security::prompt::PermissionPromptEvent::Requested(request) => {
                let _ = handle.emit("plugin-permission-request", &request);
            }
            crate::plugins::security::prompt::PermissionPromptEvent::Resolved { id } => {
                let _ = handle.emit("plugin-permission-resolved", serde_json::json!({"id": id}));
            }
        }));
    }
    #[cfg(feature = "ci-driver")]
    crate::ci_driver::start(app.handle().clone()).map_err(std::io::Error::other)?;
    #[cfg(target_os = "macos")]
    if let Some(window) = app.get_webview_window("main") {
        crate::platform::macos_material::configure(&window);
    }
    setup_tray(app.handle())?;
    std::thread::spawn(crate::core::fs::initialize_exports);
    std::thread::spawn(|| {
        crate::plugins::host::global().start_enabled();
    });
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
    Ok(())
}
