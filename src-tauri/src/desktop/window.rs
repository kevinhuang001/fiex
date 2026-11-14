use super::tray::QUITTING;
use super::{Watch, WindowPaths};
use std::sync::atomic::Ordering;
use tauri::Manager;

pub(super) fn handle_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if let tauri::WindowEvent::CloseRequested { api, .. } = event
        && window.label() == "main"
        && !QUITTING.load(Ordering::SeqCst)
    {
        api.prevent_close();
        let _ = window.hide();
        return;
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
        window
            .state::<crate::core::drag_session::DragSessions>()
            .end(window.label());
    }
}
