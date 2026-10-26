//! Locale-aware tray menu; action identities stay stable across languages.
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};
pub(super) static QUITTING: AtomicBool = AtomicBool::new(false);

fn show_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
/// Closing the window keeps Fiex alive in the tray; only the tray menu quits.
pub(super) fn setup_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let menu = menu(app)?;
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

fn menu(app: &tauri::AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let show = MenuItem::with_id(app, "show", crate::i18n::t("显示 Fiex"), true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", crate::i18n::t("退出 Fiex"), true, None::<&str>)?;
    Menu::with_items(app, &[&show, &quit])
}
pub(super) fn refresh(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(tray) = app.tray_by_id("fiex-tray") {
        tray.set_menu(Some(menu(app).map_err(|e| e.to_string())?))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
