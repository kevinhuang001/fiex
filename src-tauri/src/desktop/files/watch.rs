use super::*;

#[tauri::command]
pub(crate) fn begin_operation(state: tauri::State<'_, OperationCancel>) {
    state.begin();
}

#[tauri::command]
pub(crate) fn cancel_operation(state: tauri::State<'_, OperationCancel>) {
    state.cancel();
}

#[tauri::command]
pub(crate) fn watch_directory(
    window: tauri::Window,
    app: tauri::AppHandle,
    path: String,
    pane: String,
    state: tauri::State<'_, Watch>,
) -> Result<(), String> {
    crate::plugins::mounts::reject_local(&path)?;
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
pub(crate) fn unwatch_directory(
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
