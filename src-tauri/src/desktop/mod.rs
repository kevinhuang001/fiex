//! Tauri command surface for the desktop application.
//!
//! Commands live in domain modules (files, plugins, remote, volumes, system)
//! and are re-exported from here so `tauri::generate_handler!` entries and
//! every `crate::desktop::<command>` path keep working unchanged.
//!
//! `#[macro_use]` is required on the command modules: `#[tauri::command]`
//! exports its wrapper macro at the crate root for non-private commands, and
//! the parent module must import those macros textually for
//! `generate_handler!` to resolve them.

#[macro_use]
mod files;
#[macro_use]
mod plugins;
mod ssh;
#[macro_use]
mod remote;
#[macro_use]
mod system;
#[macro_use]
mod volumes;

pub(crate) use files::*;
pub(crate) use plugins::*;
pub(crate) use remote::*;
pub(crate) use system::*;
pub(crate) use volumes::*;

use crate::{
    drag_session::DragSessions,
    operations::{OperationCancel, Operations},
    recent,
    search::SearchState,
};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};

static QUITTING: AtomicBool = AtomicBool::new(false);

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
            plugin_context_menu_resolve,
            plugin_context_menu_execute,
            connection_open,
            vfs_retain_views,
            materialize_file,
            plugin_context_directory,
            plugin_task,
            plugin_cancel_task,
            plugin_status,
            plugins_list,
            file_info_details,
            plugin_errors,
            plugin_error_report,
            plugin_error_clear,
            plugin_panel_html,
            preview_open,
            preview_candidates,
            ssh::ssh_open,
            ssh::local_terminal_open,
            ssh::ssh_write,
            ssh::ssh_resize,
            ssh::ssh_close,
            plugin_panel_asset,
            plugin_permission_allowed,
            plugin_network_domains,
            plugin_themes,
            plugin_decorations,
            plugin_marketplace,
            system_proxy_info,
            plugin_release_assets,
            plugin_prepare_install,
            plugin_prepare_local_install,
            plugin_confirm_install,
            plugin_cancel_install,
            plugin_uninstall,
            plugin_set_enabled,
            plugin_activate,
            plugin_set_permission,
            plugin_set_command,
            plugin_set_network_domain,
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
            drag_files,
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
            cancel_search,
            begin_operation,
            cancel_operation,
            pick_folder,
            plugin_file_dialog,
            plugin_files_dialog,
            plugin_search,
            detect_terminals,
            detect_ides,
            open_in_ide,
            open_terminal,
            open_external_url,
            open_path,
            remote_connections,
            save_remote_connection,
            update_connection_session,
            remove_remote_connection,
            remote_download,
            remote_download_dir,
            remote_sync,
            remote_unsync,
            remote_resolve,
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
                crate::mounts::forget_window(window.label());
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
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let running =
                    std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()));
                loop {
                    for connection in crate::remote::list_connections() {
                        let id = connection.id;
                        if !running.lock().unwrap().insert(id.clone()) {
                            continue;
                        }
                        let running = running.clone();
                        let handle = handle.clone();
                        std::thread::spawn(move || {
                            tauri::async_runtime::block_on(crate::remote::reconcile_connection(
                                &id,
                            ));
                            use tauri::Emitter;
                            let _ = handle.emit("remote-sync", ());
                            running.lock().unwrap().remove(&id);
                        });
                    }
                    std::thread::sleep(std::time::Duration::from_secs(30));
                }
            });
            {
                use tauri::Emitter;
                let handle = app.handle().clone();
                crate::plugin_task_events::set_sink(Box::new(move |event| {
                    let _ = handle.emit("plugin-task", &event);
                }));
            }
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
        .build(tauri::generate_context!())
        .expect("Cannot start Fiex")
        .run(|_, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                crate::plugin_vfs::cleanup_exports();
            }
        });
}
