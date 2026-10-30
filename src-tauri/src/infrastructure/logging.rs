//! Process-wide, bounded execution diagnostics. Plugin and Core emitters never own files.
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_ENTRIES: usize = 500;
const MAX_MESSAGE: usize = 500;
const MAX_DETAIL: usize = 4_000;
const MAX_FILE_BYTES: u64 = 1024 * 1024;
const MAX_REPORTS_PER_MINUTE: u32 = 120;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogRecord {
    #[serde(default)]
    pub id: u64,
    pub at: u64,
    pub source: String,
    pub level: String,
    #[serde(default)]
    pub operation_id: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Default)]
struct State {
    entries: HashMap<String, VecDeque<LogRecord>>,
    rates: HashMap<String, (u64, u32)>,
}
static STATE: OnceLock<Mutex<State>> = OnceLock::new();

fn state() -> &'static Mutex<State> {
    STATE.get_or_init(|| Mutex::new(State::default()))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}
use super::redact::clean;
mod writer;

fn append(
    plugin_id: &str,
    source: &str,
    level: &str,
    message: &str,
    detail: Option<&str>,
) -> Result<(), String> {
    append_context(plugin_id, source, level, message, detail, None, None)
}
#[allow(clippy::too_many_arguments)]
fn append_context(
    plugin_id: &str,
    source: &str,
    level: &str,
    message: &str,
    detail: Option<&str>,
    operation: Option<&str>,
    duration: Option<u64>,
) -> Result<(), String> {
    if !matches!(level, "trace" | "debug" | "info" | "warn" | "error") {
        return Err("unknown log level".into());
    }
    let message = clean(message, MAX_MESSAGE);
    if message.is_empty() {
        return Err("empty log message".into());
    }
    let record = LogRecord {
        id: next_id(),
        operation_id: operation.map(|v| clean(v, 128)),
        duration_ms: duration,
        at: now_ms(),
        source: clean(source, 64),
        level: level.into(),
        message,
        detail: detail
            .map(|d| clean(d, MAX_DETAIL))
            .filter(|d| !d.is_empty()),
    };
    let mut state = state().lock().map_err(|_| "log store unavailable")?;
    if state.entries.len() >= 128 && !state.entries.contains_key(plugin_id) {
        if let Some(victim) = state.entries.keys().find(|k| k.as_str() != "core").cloned() {
            state.entries.remove(&victim);
        }
    }
    let entries = state.entries.entry(plugin_id.into()).or_default();
    entries.push_front(record.clone());
    entries.truncate(MAX_ENTRIES);
    writer::append(plugin_id, record);
    Ok(())
}
fn allowed(plugin_id: &str, level: &str) -> bool {
    if !matches!(
        level,
        "trace" | "debug" | "info" | "warn" | "error" | "stderr"
    ) {
        return false;
    }
    let now = now_ms();
    let Ok(mut state) = state().lock() else {
        return false;
    };
    if state.rates.len() >= 512 {
        state
            .rates
            .retain(|_, (at, _)| now.saturating_sub(*at) < 60_000);
    }
    if state.rates.len() >= 512 {
        return false;
    }
    let entry = state
        .rates
        .entry(format!("{plugin_id}:{level}"))
        .or_insert((now, 0));
    if now.saturating_sub(entry.0) >= 60_000 {
        *entry = (now, 0);
    }
    let limit = if matches!(level, "warn" | "error") {
        240
    } else {
        MAX_REPORTS_PER_MINUTE
    };
    if entry.1 >= limit {
        writer::dropped();
        return false;
    }
    entry.1 += 1;
    true
}
pub fn report(
    plugin_id: &str,
    source: &str,
    message: &str,
    detail: Option<&str>,
) -> Result<(), String> {
    append(plugin_id, source, "error", message, detail)
}
pub fn report_limited(plugin_id: &str, code: &str, message: &str, detail: Option<&str>) {
    if !code.is_empty()
        && (code.len() > 64
            || !code
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c)))
    {
        return;
    }
    if !allowed(plugin_id, "error") {
        return;
    }
    let message = if code.is_empty() {
        message.into()
    } else {
        format!("[{code}] {message}")
    };
    let _ = append(plugin_id, "plugin", "error", &message, detail);
}
pub fn report_log_limited(plugin_id: &str, level: &str, message: &str) {
    if !allowed(plugin_id, level) {
        return;
    }
    let _ = append(plugin_id, "plugin", level, message, None);
}
/// Capture the plugin's diagnostic channel without mixing it with JSON-RPC.
pub fn report_plugin_stderr(plugin_id: &str, message: &str) {
    if !allowed(plugin_id, "stderr") {
        return;
    }
    let _ = append(plugin_id, "stderr", "info", message, None);
}
pub fn record_core(level: &str, source: &str, message: &str) {
    let _ = append("core", source, level, message, None);
}
pub fn list(plugin_id: &str) -> Vec<LogRecord> {
    // Read history outside the memory mutex, then merge by stable process IDs.
    let mut rows = writer::load_disk(plugin_id);
    if let Ok(state) = state().lock() {
        if let Some(memory) = state.entries.get(plugin_id) {
            for record in memory.iter().rev() {
                if !rows
                    .iter()
                    .any(|old| old.id == record.id && old.at == record.at)
                {
                    rows.push_front(record.clone());
                }
            }
        }
    }
    rows.truncate(MAX_ENTRIES);
    rows.into_iter()
        .map(|mut r| {
            r.message = clean(&r.message, MAX_MESSAGE);
            r.detail = r.detail.map(|v| clean(&v, MAX_DETAIL));
            r.source = clean(&r.source, 64);
            r
        })
        .collect()
}
pub fn clear(plugin_id: &str) {
    if let Ok(mut state) = state().lock() {
        state.entries.remove(plugin_id);
        state
            .rates
            .retain(|key, _| !key.starts_with(&format!("{plugin_id}:")));
        writer::clear(plugin_id);
    }
}
fn next_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static ID: OnceLock<AtomicU64> = OnceLock::new();
    ID.get_or_init(|| AtomicU64::new(now_ms().saturating_mul(1000)))
        .fetch_add(1, Ordering::Relaxed)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogPage {
    pub entries: Vec<LogRecord>,
    pub cursor: u64,
    pub has_more: bool,
}
pub fn query(plugin_id: &str, after: u64, limit: usize) -> LogPage {
    let mut entries: Vec<_> = list(plugin_id)
        .into_iter()
        .filter(|r| r.id > after)
        .collect();
    entries.sort_by_key(|r| r.id);
    let has_more = entries.len() > limit.clamp(1, 500);
    entries.truncate(limit.clamp(1, 500));
    let cursor = entries.last().map_or(after, |r| r.id);
    entries.reverse();
    LogPage {
        entries,
        cursor,
        has_more,
    }
}

/// Finite shutdown barrier; background I/O cannot indefinitely hold up exit.
pub fn flush() {
    writer::flush();
}

pub fn operation(
    plugin_id: &str,
    component: &str,
    operation: &str,
    level: &str,
    duration: Option<u64>,
    message: &str,
) {
    let _ = append_context(
        plugin_id,
        component,
        level,
        message,
        None,
        Some(operation),
        duration,
    );
}

pub fn report_operation_limited(plugin_id: &str, level: &str, message: &str, operation_id: &str) {
    if allowed(plugin_id, level) {
        operation(plugin_id, "plugin", operation_id, level, None, message);
    }
}

#[cfg(test)]
mod tests;
