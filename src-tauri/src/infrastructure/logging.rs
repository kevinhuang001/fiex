//! Process-wide, bounded execution diagnostics. Plugin and Core emitters never own files.
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
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
    pub at: u64,
    pub source: String,
    pub level: String,
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
fn clean(value: &str, limit: usize) -> String {
    // Diagnostics never include control sequences. Avoid accidentally storing
    // access tokens in the common Authorization / Bearer header formats.
    let words = value.replace('\0', "").replace('\u{001b}', "");
    let tokens: Vec<&str> = words.split_whitespace().collect();
    let mut redacted = String::new();
    let mut hide = false;
    for word in tokens {
        if !redacted.is_empty() {
            redacted.push(' ');
        }
        if hide {
            redacted.push_str("[REDACTED]");
            hide = false;
        } else {
            redacted.push_str(word);
            hide = word.eq_ignore_ascii_case("Bearer")
                || word.eq_ignore_ascii_case("Authorization:")
                || word.eq_ignore_ascii_case("access_token:");
        }
    }
    redacted.chars().take(limit).collect()
}
fn directory() -> Option<PathBuf> {
    dirs::data_local_dir().map(|p| p.join("fiex").join("logs"))
}
fn paths(plugin_id: &str) -> Option<(PathBuf, PathBuf)> {
    use sha2::{Digest, Sha256};
    let dir = directory()?;
    let hash = hex::encode(Sha256::digest(plugin_id.as_bytes()));
    Some((
        dir.join(format!("{hash}.jsonl")),
        dir.join(format!("{hash}.previous.jsonl")),
    ))
}
fn load_disk(plugin_id: &str) -> VecDeque<LogRecord> {
    let mut rows = VecDeque::new();
    let Some((current, previous)) = paths(plugin_id) else {
        return rows;
    };
    for path in [previous, current] {
        if let Ok(content) = fs::read_to_string(path) {
            for line in content.lines() {
                if let Ok(record) = serde_json::from_str::<LogRecord>(line) {
                    rows.push_front(record);
                    rows.truncate(MAX_ENTRIES);
                }
            }
        }
    }
    rows
}
fn persist(plugin_id: &str, record: &LogRecord) {
    let Some((current, previous)) = paths(plugin_id) else {
        return;
    };
    let Some(parent) = current.parent() else {
        return;
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).is_err() {
            return;
        }
    }
    if fs::metadata(&current).is_ok_and(|m| m.len() > MAX_FILE_BYTES) {
        let _ = fs::remove_file(&previous);
        let _ = fs::rename(&current, &previous);
    }
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    if let Ok(mut file) = options.open(current) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if file.set_permissions(fs::Permissions::from_mode(0o600)).is_err() {
                return;
            }
        }
        if let Ok(json) = serde_json::to_string(record) {
            let _ = writeln!(file, "{json}");
        }
    }
}
fn append(
    plugin_id: &str,
    source: &str,
    level: &str,
    message: &str,
    detail: Option<&str>,
) -> Result<(), String> {
    if !matches!(level, "trace" | "debug" | "info" | "warn" | "error") {
        return Err("unknown log level".into());
    }
    let message = clean(message, MAX_MESSAGE);
    if message.is_empty() {
        return Err("empty log message".into());
    }
    let record = LogRecord {
        at: now_ms(),
        source: clean(source, 64),
        level: level.into(),
        message,
        detail: detail
            .map(|d| clean(d, MAX_DETAIL))
            .filter(|d| !d.is_empty()),
    };
    let mut state = state().lock().map_err(|_| "log store unavailable")?;
    let entries = state
        .entries
        .entry(plugin_id.into())
        .or_insert_with(|| load_disk(plugin_id));
    entries.push_front(record.clone());
    entries.truncate(MAX_ENTRIES);
    persist(plugin_id, &record);
    Ok(())
}
fn allowed(plugin_id: &str) -> bool {
    let now = now_ms();
    let Ok(mut state) = state().lock() else {
        return false;
    };
    let entry = state.rates.entry(plugin_id.into()).or_insert((now, 0));
    if now.saturating_sub(entry.0) >= 60_000 {
        *entry = (now, 0);
    }
    if entry.1 >= MAX_REPORTS_PER_MINUTE {
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
    if !allowed(plugin_id) {
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
    if !allowed(plugin_id) {
        return;
    }
    let _ = append(plugin_id, "plugin", level, message, None);
}
pub fn record_core(level: &str, source: &str, message: &str) {
    let _ = append("core", source, level, message, None);
}
pub fn list(plugin_id: &str) -> Vec<LogRecord> {
    state()
        .lock()
        .map(|mut state| {
            state
                .entries
                .entry(plugin_id.into())
                .or_insert_with(|| load_disk(plugin_id))
                .iter()
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}
pub fn clear(plugin_id: &str) {
    if let Ok(mut state) = state().lock() {
        state.entries.remove(plugin_id);
        state.rates.remove(plugin_id);
        if let Some((current, previous)) = paths(plugin_id) {
            let _ = fs::remove_file(current);
            let _ = fs::remove_file(previous);
        }
    }
}
