use serde_json::Value;
#[tauri::command]
pub(crate) fn state_get(namespace: String, key: String) -> Result<Option<Value>, String> {
    crate::infrastructure::state::get(&namespace, &key)
}
#[tauri::command]
pub(crate) fn state_put(namespace: String, key: String, value: Value) -> Result<(), String> {
    crate::infrastructure::state::put(&namespace, &key, value)
}
#[tauri::command]
pub(crate) fn state_remove(namespace: String, key: String) -> Result<(), String> {
    crate::infrastructure::state::remove(&namespace, &key)
}
