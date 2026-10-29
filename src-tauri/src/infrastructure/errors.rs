//! One centralized user-notification policy. Execution diagnostics live in
//! LogCenter; this queue contains only errors worth presenting to the user.
use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, sync::{Mutex, OnceLock}, time::{SystemTime, UNIX_EPOCH}};

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
}
#[derive(Default)]
struct Center {
    next_id: u64,
    items: VecDeque<AppError>,
    last: Option<(u64, String, String, String)>,
}
static ERRORS: OnceLock<Mutex<Center>> = OnceLock::new();
fn center() -> &'static Mutex<Center> { ERRORS.get_or_init(|| Mutex::new(Center::default())) }
fn now() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).map(|v| v.as_millis() as u64).unwrap_or_default() }
fn sanitized(value: &str, limit: usize) -> String {
    value.chars().filter(|c| !c.is_control() || *c == '\n').take(limit).collect::<String>().trim().to_string()
}
fn expected(code: &str, message: &str) -> bool {
    let combined = format!("{code} {message}").to_ascii_lowercase();
    ["cancelled", "canceled", "unsupported", "not implemented", "no suitable provider",
     "permission request", "activation pending", "fallback", "permissiondenied"]
        .iter().any(|text| combined.contains(text))
}
/// A Core-controlled notification request. Plugins cannot set severity or forge
/// sources; the host supplies their registered plugin ID.
pub fn report(source: &str, code: &str, message: &str, operation: Option<&str>, retryable: bool) {
    if expected(code, message) { return; }
    let source = sanitized(source, 128);
    let code = sanitized(code, 64);
    let message = sanitized(message, 1500);
    if source.is_empty() || message.is_empty() { return; }
    let at = now();
    let Ok(mut state) = center().lock() else { return };
    if state.last.as_ref().is_some_and(|(last, s, c, m)|
        at.saturating_sub(*last) < 5000 && s == &source && c == &code && m == &message) {
        return;
    }
    state.last = Some((at, source.clone(), code.clone(), message.clone()));
    state.next_id = state.next_id.saturating_add(1);
    let id = state.next_id;
    state.items.push_back(AppError {
        id, at, source, code, message, operation: operation.map(|v| sanitized(v, 128)),
        retryable,
    });
    while state.items.len() > 50 { state.items.pop_front(); }
}
/// GUI drains the notification queue; Core errors remain in the LogCenter.
pub fn drain() -> Vec<AppError> {
    center().lock().map(|mut center| center.items.drain(..).collect()).unwrap_or_default()
}
