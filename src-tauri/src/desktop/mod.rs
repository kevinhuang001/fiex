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
mod credentials;
#[macro_use]
mod local_terminal;
#[macro_use]
mod files;
#[macro_use]
mod locale;
mod tray;
use locale::locale_set;
use tray::{setup_tray, QUITTING};
mod scheduler;
#[macro_use]
mod plugins;
pub(crate) mod shell_profile;
pub(crate) mod terminal;
#[macro_use]
mod remote;
#[macro_use]
mod system;
#[macro_use]
mod volumes;

pub(crate) use credentials::*;
pub(crate) use files::*;
pub(crate) use local_terminal::*;
pub(crate) use plugins::*;
pub(crate) use remote::*;
pub(crate) use system::*;
pub(crate) use volumes::*;

use crate::{
    core::drag_session::DragSessions,
    core::operations::{OperationCancel, Operations},
    core::recent,
    core::search::SearchState,
};
use std::sync::atomic::Ordering;
use tauri::Manager;

#[tauri::command]
pub(crate) fn system_fonts() -> Vec<String> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let mut names = std::collections::BTreeSet::new();
    for face in db.faces() {
        for (family, _) in &face.families {
            if !family.is_empty() {
                names.insert(family.clone());
            }
        }
    }
    names.into_iter().collect()
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // A second launch focuses the existing window instead of starting
            // another Core process and a second set of plugin children.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
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
            locale_set,
            plugin_invoke,
            plugin_describe_resources,
            plugin_before_open,
            plugin_toolbar_invoke,
            plugin_context_menu_resolve,
            plugin_context_menu_children,
            plugin_context_menu_execute,
            plugin_connection_menu_resolve,
            plugin_connection_menu_execute,
            connection_open,
            vfs_retain_views,
            plugin_context_directory,
            plugin_task,
            plugin_cancel_task,
            plugin_status,
            system_fonts,
            plugins_list,
            plugin_settings,
            file_info_details,
            plugin_errors,
            plugin_error_report,
            plugin_error_clear,
            plugin_panel_html,
            preview_open,
            preview_candidates,
            preview_loading,
            preview_loading_asset,
            local_terminal_open,
            detect_shells,
            terminal::terminal_write,
            terminal::terminal_resize,
            terminal::terminal_close,
            plugin_panel_asset,
            plugin_view_asset,
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
            plugin_permission_respond,
            plugin_set_command,
            plugin_set_network_all,
            plugin_set_network_domain,
            plugin_approve_install,
            plugin_storage_get,
            plugin_storage_set,
            plugin_storage_delete,
            plugin_secret_set,
            plugin_secret_available,
            credential_backend,
            credential_backend_set,
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
            virtual_cache,
            file_info,
            read_preview,
            create_entry,
            create_file_entry,
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
            search_providers,
            provider_catalog,
            provider_preferences,
            set_provider_preference,
            reset_provider_preference,
            detect_terminals,
            detect_ides,
            open_in_ide,
            open_terminal,
            open_external_url,
            open_path,
            remote_connections,
            remote_connection_config,
            save_remote_connection,
            update_connection_session,
            remove_remote_connection,
            remote_download,
            remote_download_dir,
            remote_cache,
            remote_open,
            remote_sync,
            remote_uncache,
            remote_unpin,
            remote_set_sync_interval,
            remote_resolve,
            eject_device,
            eject_usb,
            relaunch_elevated,
            mount_device,
            reveal_path,
            trash_application_entries,
            remove_application_entries,
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
                crate::plugins::mounts::forget_window(window.label());
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
            scheduler::spawn(app.handle().clone());
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
                        let _ = handle
                            .emit("plugin-permission-resolved", serde_json::json!({"id": id}));
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
            // Fill the three virtual-root caches once, off the main thread;
            // later list/search calls only read them.
            std::thread::spawn(crate::core::volumes::virtual_storage::warm_caches);
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
            let _ = app;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Cannot start Fiex")
        .run(|_, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                crate::plugins::host::global().shutdown();
                crate::plugins::vfs::cleanup_exports();
            }
        });
}
