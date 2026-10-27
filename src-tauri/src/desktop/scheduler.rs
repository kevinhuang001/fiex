//! Background connection reconciliation. One round per configured interval;
//! each connection runs independently so a slow provider cannot stall others.

use tauri::{AppHandle, Emitter};

pub(crate) fn spawn(handle: AppHandle) {
    std::thread::spawn(move || {
        let running = std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()));
        loop {
            for connection in crate::remote::list_connections() {
                let id = connection.id;
                let name = connection.name;
                if !running.lock().unwrap().insert(id.clone()) {
                    continue;
                }
                let running = running.clone();
                let handle = handle.clone();
                std::thread::spawn(move || {
                    let changes =
                        tauri::async_runtime::block_on(crate::remote::reconcile_connection(&id));
                    if !changes.is_empty() {
                        let _ = handle.emit(
                            "remote-cache-state",
                            serde_json::json!({
                                "connection": id,
                                "name": name,
                                "changes": changes,
                            }),
                        );
                    }
                    let _ = handle.emit("remote-sync", ());
                    running.lock().unwrap().remove(&id);
                });
            }
            crate::remote::wait_for_round();
        }
    });
}
