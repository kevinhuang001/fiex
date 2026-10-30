//! One centralized user-notification policy. Execution diagnostics live in
//! LogCenter; this queue contains only errors worth presenting to the user.
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub id: u64,
    pub at: u64,
    pub source: String,
    pub code: String,
    pub message: String,
    pub operation: Option<String>,
    pub retryable: bool,
    pub occurrences: u32,
    pub last_at: u64,
}
#[derive(Default)]
struct Center {
    next_id: u64,
    items: VecDeque<AppError>,
}
static ERRORS: OnceLock<Mutex<Center>> = OnceLock::new();
fn center() -> &'static Mutex<Center> {
    ERRORS.get_or_init(|| Mutex::new(Center::default()))
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_millis() as u64)
        .unwrap_or_default()
}
use super::redact::clean as sanitized;
/// Only trusted, exact codes carry control-flow meaning. Provider text is never inspected.
#[derive(Clone, Copy)]
pub enum ErrorKind {
    Failure,
    Cancelled,
    Fallback,
    AuthorizationPending,
}
fn kind(code: &str) -> ErrorKind {
    match code {
        "core.cancelled" => ErrorKind::Cancelled,
        "core.fallback" => ErrorKind::Fallback,
        "core.authorization_pending" => ErrorKind::AuthorizationPending,
        _ => ErrorKind::Failure,
    }
}
/// A Core-controlled notification request. Plugins cannot set severity or forge
/// sources; the host supplies their registered plugin ID.
pub fn report(source: &str, code: &str, message: &str, operation: Option<&str>, _retryable: bool) {
    if !matches!(kind(code), ErrorKind::Failure) {
        return;
    }
    let source = sanitized(source, 128);
    let code = sanitized(code, 64);
    let message = sanitized(message, 1500);
    if source.is_empty() || message.is_empty() {
        return;
    }
    let at = now();
    let Ok(mut state) = center().lock() else {
        return;
    };
    state
        .items
        .retain(|item| at.saturating_sub(item.last_at) < 5 * 60_000);
    if let Some(existing) = state.items.iter_mut().find(|item| {
        item.source == source
            && item.code == code
            && item.message == message
            && item.operation.as_deref() == operation
    }) {
        existing.occurrences = existing.occurrences.saturating_add(1);
        existing.last_at = at;
        return;
    }
    // Independent per-source notification budget; repeated errors aggregate above.
    if state
        .items
        .iter()
        .filter(|item| item.source == source && at.saturating_sub(item.at) < 60_000)
        .count()
        >= 5
    {
        return;
    }
    state.next_id = state.next_id.saturating_add(1);
    let id = state.next_id;
    super::logging::record_core("error", &source, &format!("error:{id} [{code}] {message}"));
    state.items.push_back(AppError {
        id,
        at,
        source,
        code,
        message,
        operation: operation.map(|v| sanitized(v, 128)),
        retryable: false, // No retry action without a registered handler.
        occurrences: 1,
        last_at: at,
    });
    while state.items.len() > 50 {
        state.items.pop_front();
    }
}
/// Non-destructive cursor reads: one window can never steal another's errors.
/// Retention is bounded to 50 events and five minutes, independent of consumers.
pub fn pending(after: u64) -> Vec<AppError> {
    center()
        .lock()
        .map(|center| {
            center
                .items
                .iter()
                .filter(|item| item.id > after && now().saturating_sub(item.last_at) < 5 * 60_000)
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_readers_receive_errors_and_provider_text_is_not_policy() {
        report(
            "test:windows",
            "plugin.runtime",
            "unsupported file failed",
            None,
            false,
        );
        let first = pending(0)
            .into_iter()
            .find(|v| v.source == "test:windows")
            .unwrap();
        let second = pending(0).into_iter().find(|v| v.id == first.id).unwrap();
        assert_eq!(first.id, second.id);
        assert!(!pending(first.id).iter().any(|v| v.id == first.id));
        report(
            "test:windows",
            "plugin.runtime",
            "unsupported file failed",
            None,
            false,
        );
        assert_eq!(
            pending(0)
                .into_iter()
                .find(|v| v.id == first.id)
                .unwrap()
                .occurrences,
            2
        );
    }
}
