#[tauri::command]
pub(crate) fn credential_backend() -> Result<String, String> {
    crate::plugins::storage::credential_backend()
}
#[tauri::command]
pub(crate) async fn credential_backend_set(backend: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::plugins::storage::set_credential_backend(&backend)
    })
    .await
    .map_err(|e| e.to_string())?
}
