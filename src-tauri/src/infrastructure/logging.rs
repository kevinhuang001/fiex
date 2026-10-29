//! Central structured execution log for Core and authenticated plugin processes.
//! Backends are bounded independently of business services; a logging failure
//! must never prevent the underlying operation from completing.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, VecDeque},
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{mpsc, Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_IN_MEMORY: usize = 800;
const MAX_LINE: usize = 4096;
const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_ROTATED_FILES: usize = 3;
const MAX_PLUGIN_FIELDS: usize = 2048;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogRecord {
    pub at: u64,
    pub level: String,
    pub source: String,
    pub plugin_id: Option<String>,
    pub operation: Option<String>,
    pub message: String,
    pub fields: Value,
}

type Queues = HashMap<String, VecDeque<LogRecord>>;
fn queues() -> &'static Mutex<Queues> {
    static QUEUES: OnceLock<Mutex<Queues>> = OnceLock::new();
    QUEUES.get_or_init(|| Mutex::new(HashMap::new()))
}
fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}
fn storage_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|p| p.join("fiex").join("logs"))
}
fn storage_path(owner: &str) -> Option<PathBuf> {
    storage_dir().map(|p| p.join(format!("{owner}.jsonl")))
}
fn load(owner: &str) -> VecDeque<LogRecord> {
    let mut rows = VecDeque::new();
    // The newest active file is deliberately bounded by rotation.
    if let Some(path) = storage_path(owner) {
        if let Ok(bytes) = fs::read_to_string(path) {
            for line in bytes.lines() {
                if let Ok(record) = serde_json::from_str::<LogRecord>(line) {
                    rows.push_back(record);
                    if rows.len() > MAX_IN_MEMORY {
                        rows.pop_front();
                    }
                }
            }
        }
    }
    rows
}
fn writer() -> &'static mpsc::SyncSender<(String, LogRecord)> {
    static WRITER: OnceLock<mpsc::SyncSender<(String, LogRecord)>> = OnceLock::new();
    WRITER.get_or_init(|| {
        let (tx, rx) = mpsc::sync_channel(2048);
        std::thread::Builder::new()
            .name("fiex-log-writer".into())
            .spawn(move || {
                while let Ok((owner, record)) = rx.recv() {
                    let Some(path) = storage_path(&owner) else {
                        continue;
                    };
                    let Some(root) = path.parent() else { continue };
                    if fs::create_dir_all(root).is_err() {
                        continue;
                    }
                    let line = match serde_json::to_string(&record) {
                        Ok(line) => line,
                        Err(_) => continue,
                    };
                    if fs::metadata(&path)
                        .is_ok_and(|m| m.len() + line.len() as u64 > MAX_FILE_BYTES)
                    {
                        for n in (1..MAX_ROTATED_FILES).rev() {
                            let from = path.with_extension(format!("jsonl.{n}"));
                            let to = path.with_extension(format!("jsonl.{}", n + 1));
                            let _ = fs::rename(from, to);
                        }
                        let _ = fs::rename(&path, path.with_extension("jsonl.1"));
                    }
                    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
                        let _ = writeln!(file, "{line}");
                    }
                }
            })
            .expect("log writer thread");
        tx
    })
}
fn bounded(input: &str, max: usize) -> String {
    input.chars().filter(|c| *c != '\0').take(max).collect()
}
fn safe_fields(value: Value) -> Value {
    let Value::Object(fields) = value else {
        return Value::Null;
    };
    let mut clean = serde_json::Map::new();
    for (key, value) in fields.into_iter().take(32) {
        let lowered = key.to_ascii_lowercase();
        if [
            "password",
            "secret",
            "token",
            "cookie",
            "authorization",
            "key",
            "credential",
        ]
        .iter()
        .any(|part| lowered.contains(part))
        {
            continue;
        }
        if let Some(text) = value.as_str() {
            clean.insert(bounded(&key, 64), Value::String(bounded(text, 256)));
        } else if value.is_boolean() || value.is_number() {
            clean.insert(bounded(&key, 64), value);
        }
    }
    let safe = Value::Object(clean);
    if safe.to_string().len() > MAX_PLUGIN_FIELDS {
        Value::Null
    } else {
        safe
    }
}
fn add(owner: &str, record: LogRecord) {
    if let Ok(mut all) = queues().lock() {
        let items = all.entry(owner.to_owned()).or_insert_with(|| load(owner));
        items.push_back(record.clone());
        if items.len() > MAX_IN_MEMORY {
            items.pop_front();
        }
    }
    let _ = writer().try_send((owner.to_owned(), record));
}
pub fn core(level: &str, operation: &str, message: &str) {
    if !allowed_level(level) {
        return;
    }
    add(
        "core",
        LogRecord {
            at: timestamp(),
            level: level.into(),
            source: "core".into(),
            plugin_id: None,
            operation: Some(bounded(operation, 120)),
            message: bounded(message, MAX_LINE),
            fields: Value::Null,
        },
    );
}
fn allowed_level(level: &str) -> bool {
    matches!(level, "trace" | "debug" | "info" | "warn" | "error")
}
pub fn plugin(
    id: &str,
    level: &str,
    source: &str,
    operation: Option<&str>,
    message: &str,
    fields: Value,
) {
    if !valid_id(id) || !allowed_level(level) || message.trim().is_empty() {
        return;
    }
    let name = format!("plugin-{id}");
    add(
        &name,
        LogRecord {
            at: timestamp(),
            level: level.into(),
            source: bounded(source, 40),
            plugin_id: Some(id.into()),
            operation: operation.map(|s| bounded(s, 120)),
            message: bounded(message, MAX_LINE),
            fields: safe_fields(fields),
        },
    );
}
pub fn list(plugin_id: Option<&str>, level: Option<&str>, limit: usize) -> Vec<LogRecord> {
    let owner = match plugin_id {
        Some(id) if valid_id(id) => format!("plugin-{id}"),
        Some(_) => return Vec::new(),
        None => "core".into(),
    };
    let Ok(mut all) = queues().lock() else {
        return Vec::new();
    };
    let rows = all.entry(owner.clone()).or_insert_with(|| load(&owner));
    rows.iter()
        .rev()
        .filter(|r| level.is_none_or(|l| r.level == l))
        .take(limit.clamp(1, MAX_IN_MEMORY))
        .cloned()
        .collect()
}
pub fn clear(plugin_id: Option<&str>) {
    let owner = match plugin_id {
        Some(id) if valid_id(id) => format!("plugin-{id}"),
        Some(_) => return,
        None => "core".into(),
    };
    if let Ok(mut all) = queues().lock() {
        all.insert(owner.clone(), VecDeque::new());
    }
    if let Some(path) = storage_path(&owner) {
        let _ = fs::remove_file(&path);
        for i in 1..=MAX_ROTATED_FILES {
            let _ = fs::remove_file(path.with_extension(format!("jsonl.{i}")));
        }
    }
}
pub fn report_plugin_error(id: &str, source: &str, message: &str, detail: Option<&str>) {
    plugin(
        id,
        "error",
        source,
        None,
        message,
        detail
            .map(|d| json!({"detail": bounded(d, 1024)}))
            .unwrap_or(Value::Null),
    );
}
