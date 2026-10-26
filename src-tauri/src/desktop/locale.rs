#[tauri::command]
pub(crate) fn locale_set(app: tauri::AppHandle, locale: String) -> Result<(), String> {
    crate::i18n::set_locale(&locale)?;
    super::tray::refresh(&app)
}
