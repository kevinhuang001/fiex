//! Deprecated call-site facade over the single structured LogCenter.
//! This file owns no cache or retention policy.
use serde::Serialize;
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginError {
    pub at: u64,
    pub source: String,
    pub message: String,
    pub detail: Option<String>,
}
pub fn report(id: &str, source: &str, message: &str, detail: Option<&str>) -> Result<(), String> {
    crate::infrastructure::logging::report_plugin_error(id, source, message, detail);
    Ok(())
}
pub fn report_limited(id: &str, code: &str, message: &str, detail: Option<&str>) {
    let message = if code.is_empty() { message.to_owned() }
        else { format!("[{}] {}", code.chars().take(64).collect::<String>(), message) };
    crate::infrastructure::logging::report_plugin_error(id, "sdk", &message, detail);
}
pub fn list(id: &str) -> Vec<PluginError> {
    crate::infrastructure::logging::list(Some(id), Some("error"), 50)
        .into_iter().map(|row| PluginError {
            at: row.at, source: row.source, message: row.message,
            detail: row.fields.get("detail").and_then(|v| v.as_str()).map(str::to_owned),
        }).collect()
}
pub fn clear(id: &str) { crate::infrastructure::logging::clear(Some(id)); }
