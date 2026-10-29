//! Recent failures reported by a plugin's UI or recorded by Core.
//!
//! The log is deliberately small and in-memory: it answers "what went wrong
//! with this plugin just now?" in Settings, and must never grow without bound
//! or persist user content. Plugin UI reports arrive through the panel bridge
//! (`plugin.error`) and Core adds its own panel-load failures.
use serde::Serialize;
use std::{
    collections::{HashMap, VecDeque},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_ENTRIES: usize = 500;
const MAX_MESSAGE: usize = 500;
const MAX_DETAIL: usize = 4000;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginError {
    /// Unix epoch milliseconds.
    pub at: u64,
    /// `ui`, `bridge`, `panel` or `host`.
    pub source: String,
    pub level: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

static LOG: OnceLock<Mutex<HashMap<String, VecDeque<PluginError>>>> = OnceLock::new();

fn log() -> &'static Mutex<HashMap<String, VecDeque<PluginError>>> {
    LOG.get_or_init(|| Mutex::new(HashMap::new()))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn clean(value: &str) -> String {
    value.replace('\u{0}', "").trim().to_string()
}

fn truncate(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
}

/// Records one failure. Unknown sources are folded into `ui` so the setting
/// page never renders an arbitrary string from a plugin.
pub fn report(
    plugin_id: &str,
    source: &str,
    message: &str,
    detail: Option<&str>,
) -> Result<(), String> {
    let message = truncate(&clean(message), MAX_MESSAGE);
    if message.is_empty() {
        return Err("empty error message".into());
    }
    let source = match source {
        "bridge" | "panel" | "host" => source,
        _ => "ui",
    };
    let detail = detail
        .map(clean)
        .filter(|value| !value.is_empty())
        .map(|value| truncate(&value, MAX_DETAIL));
    let mut logs = log()
        .lock()
        .map_err(|_| "error log unavailable".to_string())?;
    let entries = logs.entry(plugin_id.to_string()).or_default();
    entries.push_front(PluginError {
        at: now_ms(),
        source: source.to_string(),
        level: "error".to_string(),
        message,
        detail,
    });
    entries.truncate(MAX_ENTRIES);
    Ok(())
}

/// Backend-reported diagnostics are fire-and-forget: a plugin must never fail
/// because the log is unavailable or because it is being throttled.
const MAX_REPORTS_PER_MINUTE: u32 = 120;
static RATE: OnceLock<Mutex<HashMap<String, (u64, u32)>>> = OnceLock::new();

fn rate() -> &'static Mutex<HashMap<String, (u64, u32)>> {
    RATE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn valid_code(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= 64
        && code
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
}

fn allowed(plugin_id: &str, now: u64) -> bool {
    let Ok(mut rates) = rate().lock() else {
        return false;
    };
    let entry = rates.entry(plugin_id.to_string()).or_insert((now, 0));
    if now.saturating_sub(entry.0) >= 60_000 {
        *entry = (now, 0);
    }
    if entry.1 >= MAX_REPORTS_PER_MINUTE {
        return false;
    }
    entry.1 += 1;
    true
}

pub fn report_limited(plugin_id: &str, code: &str, message: &str, detail: Option<&str>) {
    let message = clean(message);
    if message.is_empty() {
        return;
    }
    if !code.is_empty() && !valid_code(code) {
        return;
    }
    let now = now_ms();
    if !allowed(plugin_id, now) {
        return;
    }
    let text = if code.is_empty() {
        message
    } else {
        format!("[{code}] {message}")
    };
    let _ = report(plugin_id, "plugin", &text, detail);
}


pub fn report_log_limited(plugin_id: &str, level: &str, message: &str) {
    if !matches!(level, "trace" | "debug" | "info" | "warn" | "error") {
        return;
    }
    let message = truncate(&clean(message), MAX_MESSAGE);
    if message.is_empty() {
        return;
    }
    let now = now_ms();
    if !allowed(plugin_id, now) {
        return;
    }
    if let Ok(mut logs) = log().lock() {
        let entries = logs.entry(plugin_id.to_string()).or_default();
        entries.push_front(PluginError {
            at: now,
            source: "plugin".to_string(),
            level: level.to_string(),
            message,
            detail: None,
        });
        entries.truncate(MAX_ENTRIES);
    }
}

/// Newest first.
pub fn list(plugin_id: &str) -> Vec<PluginError> {
    log()
        .lock()
        .ok()
        .and_then(|logs| logs.get(plugin_id).cloned())
        .map(|entries| entries.into_iter().collect())
        .unwrap_or_default()
}

pub fn clear(plugin_id: &str) {
    if let Ok(mut logs) = log().lock() {
        logs.remove(plugin_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_newest_first_and_caps_the_log() {
        let id = "test.errors.cap";
        clear(id);
        for index in 0..(MAX_ENTRIES + 10) {
            report(id, "ui", &format!("failure {index}"), None).unwrap();
        }
        let entries = list(id);
        assert_eq!(entries.len(), MAX_ENTRIES);
        assert_eq!(entries[0].message, format!("failure {}", MAX_ENTRIES + 9));
        assert_eq!(entries[entries.len() - 1].message, "failure 10");
    }

    #[test]
    fn rejects_empty_messages_and_folds_unknown_sources() {
        let id = "test.errors.validation";
        clear(id);
        assert!(report(id, "ui", "   ", None).is_err());
        assert!(report(id, "ui", "\0", None).is_err());
        report(id, "definitely-not-a-source", "boom", Some("stack")).unwrap();
        let entries = list(id);
        assert_eq!(entries[0].source, "ui");
        assert_eq!(entries[0].detail.as_deref(), Some("stack"));
    }

    #[test]
    fn backend_reports_are_validated_and_capped_per_minute() {
        let id = "test.errors.rate";
        clear(id);
        let before = list(id).len();
        report_limited(id, "gpg.no_key", "no key", None);
        report_limited(id, "bad code!", "rejected", None);
        report_limited(id, "", "no code", None);
        for index in 0..20 {
            report_limited(id, "gpg.retry", &format!("retry {index}"), None);
        }
        let entries = list(id);
        assert_eq!(entries.len() - before, MAX_REPORTS_PER_MINUTE as usize);
        assert_eq!(entries[0].message, "[gpg.retry] retry 7");
        clear(id);
    }

    #[test]
    fn truncates_long_text_and_isolates_plugins() {
        let long_message = "x".repeat(2000);
        let long_detail = "y".repeat(10000);
        report(
            "test.errors.long",
            "bridge",
            &long_message,
            Some(&long_detail),
        )
        .unwrap();
        let entries = list("test.errors.long");
        assert_eq!(entries[0].message.chars().count(), MAX_MESSAGE);
        assert_eq!(
            entries[0].detail.as_deref().unwrap().chars().count(),
            MAX_DETAIL
        );
        assert!(list("test.errors.other").is_empty());
        clear("test.errors.long");
    }
}
