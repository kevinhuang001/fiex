//! Versioned, atomic user state persistence, independently of cache eviction.
use serde_json::{Map, Value};
use std::{
    fs,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};
const MAX_STORE: usize = 2 * 1024 * 1024;
const MAX_VALUE: usize = 128 * 1024;
fn path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("fiex").join("state.json"))
}
fn state() -> &'static Mutex<Map<String, Value>> {
    static STATE: OnceLock<Mutex<Map<String, Value>>> = OnceLock::new();
    STATE.get_or_init(|| {
        let saved = path()
            .and_then(|p| fs::read(p).ok())
            .filter(|s| s.len() <= MAX_STORE)
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .and_then(|v| v.get("values").and_then(Value::as_object).cloned())
            .unwrap_or_default();
        Mutex::new(saved)
    })
}
fn valid(key: &str) -> bool {
    key.len() <= 160
        && !key.is_empty()
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b':'))
}
pub fn get(key: &str) -> Option<Value> {
    if !valid(key) {
        return None;
    }
    state().lock().ok()?.get(key).cloned()
}
pub fn set(key: &str, value: Value) -> Result<(), String> {
    if !valid(key) {
        return Err("invalid state key".into());
    }
    if serde_json::to_vec(&value).map_err(|e| e.to_string())?.len() > MAX_VALUE {
        return Err("state value too large".into());
    }
    let mut entries = state().lock().map_err(|_| "state store unavailable")?;
    // Commit memory only after the new disk snapshot is safely persisted.
    let mut candidate = entries.clone();
    candidate.insert(key.into(), value);
    let bytes = serde_json::to_vec(&serde_json::json!({"version":1,"values":&candidate}))
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_STORE {
        return Err("state store full".into());
    }
    let target = path().ok_or("configuration directory unavailable")?;
    let parent = target.parent().ok_or("configuration parent unavailable")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    use std::io::Write;
    let mut tempfile = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    tempfile.write_all(&bytes).map_err(|e| e.to_string())?;
    tempfile.as_file().sync_all().map_err(|e| e.to_string())?;
    tempfile.persist(&target).map_err(|e| e.error.to_string())?;
    *entries = candidate;
    Ok(())
}
