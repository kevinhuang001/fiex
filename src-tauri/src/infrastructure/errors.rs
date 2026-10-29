//! User-facing error decisions are separate from execution log storage.
use serde::Serialize;
use std::{
    collections::VecDeque,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub id: u64,
    pub at: u64,
    pub code: String,
    pub message: String,
    pub operation: String,
    pub source: String,
}
fn pending() -> &'static Mutex<VecDeque<AppError>> {
    static PENDING: OnceLock<Mutex<VecDeque<AppError>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(VecDeque::new()))
}
pub fn report(code: &str, operation: &str, source: &str, message: &str, notify: bool) {
    crate::infrastructure::logging::core("error", operation, message);
    if !notify || message.trim().is_empty() {
        return;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default();
    if let Ok(mut queue) = pending().lock() {
        // Coalesce bursts from failing providers instead of opening modal storms.
        if queue
            .iter()
            .any(|e| e.code == code && e.operation == operation && e.message == message)
        {
            return;
        }
        queue.push_back(AppError {
            id: now,
            at: now,
            code: code.chars().take(100).collect(),
            operation: operation.chars().take(128).collect(),
            source: source.chars().take(64).collect(),
            message: message.chars().take(2000).collect(),
        });
        while queue.len() > 32 {
            queue.pop_front();
        }
    }
}
pub fn drain() -> Vec<AppError> {
    pending()
        .lock()
        .map(|mut q| q.drain(..).collect())
        .unwrap_or_default()
}
