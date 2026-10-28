//! Host-owned per-plugin persistent storage and credential broker.
//! No plugin-controlled absolute paths; all write operations are quota limited.
//! APIs exposed to plugins MUST be routed through a verified session identity.
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

const MAX_VALUE_BYTES: usize = 1024 * 1024;
const MAX_SCOPE_BYTES: usize = 16 * 1024 * 1024;

fn valid_segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        && value != "."
        && value != ".."
        && !value.starts_with('.')
}
pub(super) fn validate(plugin: &str, scope: &str, key: &str) -> Result<(), String> {
    if !valid_segment(plugin) || !valid_segment(scope) || !valid_segment(key) {
        return Err("invalid plugin, scope or key".into());
    }
    Ok(())
}
fn base() -> Result<PathBuf, String> {
    dirs::data_local_dir()
        .ok_or("application data directory unavailable".to_string())
        .map(|d| d.join("fiex").join("plugin-state"))
}
fn file(plugin: &str, scope: &str) -> Result<PathBuf, String> {
    if !valid_segment(plugin) || !valid_segment(scope) {
        return Err("invalid plugin scope".into());
    }
    Ok(base()?.join(plugin).join(format!("{scope}.json")))
}
fn mutex() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}
fn load(path: &Path) -> Result<BTreeMap<String, Value>, String> {
    match fs::read(path) {
        Ok(data) => {
            serde_json::from_slice(&data).map_err(|e| format!("plugin storage corrupted: {e}"))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(e) => Err(e.to_string()),
    }
}
fn save(path: &Path, items: &BTreeMap<String, Value>) -> Result<(), String> {
    let data = serde_json::to_vec(items).map_err(|e| e.to_string())?;
    if data.len() > MAX_SCOPE_BYTES {
        return Err("plugin storage quota exceeded".into());
    }
    let parent = path.parent().ok_or("invalid plugin storage directory")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    use std::io::Write;
    temp.write_all(&data).map_err(|error| error.to_string())?;
    temp.as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    temp.persist(path).map_err(|error| error.to_string())?;
    Ok(())
}

fn compare_at(
    path: &Path,
    key: &str,
    expected: Option<&Value>,
    next: Option<Value>,
) -> Result<bool, String> {
    let mut items = load(path)?;
    if items.get(key) != expected {
        return Ok(false);
    }
    if let Some(value) = next {
        items.insert(key.into(), value);
    } else {
        items.remove(key);
    }
    save(path, &items)?;
    Ok(true)
}
/// One compare and replacement under the same Core lock, including a
/// distinction between an absent key and a stored JSON null.
pub fn compare_and_set(
    plugin: &str,
    scope: &str,
    key: &str,
    expected: Option<Value>,
    next: Option<Value>,
) -> Result<bool, String> {
    validate(plugin, scope, key)?;
    if let Some(value) = &next {
        if serde_json::to_vec(value)
            .map_err(|error| error.to_string())?
            .len()
            > MAX_VALUE_BYTES
        {
            return Err("plugin storage value too large".into());
        }
    }
    let _guard = mutex().lock().map_err(|_| "plugin storage lock poisoned")?;
    compare_at(&file(plugin, scope)?, key, expected.as_ref(), next)
}

pub fn get(plugin: &str, scope: &str, key: &str) -> Result<Option<Value>, String> {
    validate(plugin, scope, key)?;
    let _guard = mutex().lock().map_err(|_| "plugin storage lock poisoned")?;
    Ok(load(&file(plugin, scope)?)?.remove(key))
}
pub fn set(plugin: &str, scope: &str, key: &str, value: Value) -> Result<(), String> {
    validate(plugin, scope, key)?;
    if serde_json::to_vec(&value).map_err(|e| e.to_string())?.len() > MAX_VALUE_BYTES {
        return Err("plugin storage value too large".into());
    }
    let _guard = mutex().lock().map_err(|_| "plugin storage lock poisoned")?;
    let path = file(plugin, scope)?;
    let mut items = load(&path)?;
    items.insert(key.into(), value);
    save(&path, &items)
}
pub fn delete(plugin: &str, scope: &str, key: &str) -> Result<(), String> {
    validate(plugin, scope, key)?;
    let _guard = mutex().lock().map_err(|_| "plugin storage lock poisoned")?;
    let path = file(plugin, scope)?;
    let mut items = load(&path)?;
    items.remove(key);
    save(&path, &items)
}
pub fn list_keys(plugin: &str, scope: &str) -> Result<Vec<String>, String> {
    if !valid_segment(plugin) || !valid_segment(scope) {
        return Err("invalid plugin scope".into());
    }
    let _guard = mutex().lock().map_err(|_| "plugin storage lock poisoned")?;
    Ok(load(&file(plugin, scope)?)?.into_keys().collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_traversal() {
        assert!(set("../evil", "global", "safe", Value::Null).is_err());
        assert!(set("valid.plugin", "../x", "safe", Value::Null).is_err());
        assert!(set("valid.plugin", "global", "../x", Value::Null).is_err());
    }
    #[test]
    fn size_cap() {
        assert!(set(
            "valid.plugin",
            "global",
            "test",
            Value::String("a".repeat(MAX_VALUE_BYTES + 1))
        )
        .is_err());
    }
}

#[cfg(test)]
mod atomic_tests {
    use super::*;
    #[test]
    fn cas_distinguishes_absent_null_and_conflicts() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("state.json");
        assert!(compare_at(&path, "key", None, Some(Value::Null)).unwrap());
        assert!(!compare_at(&path, "key", None, Some(Value::Bool(true))).unwrap());
        assert!(compare_at(&path, "key", Some(&Value::Null), Some(Value::from(1))).unwrap());
        assert!(!compare_at(&path, "key", Some(&Value::Null), None).unwrap());
        assert_eq!(load(&path).unwrap()["key"], Value::from(1));
    }
    #[test]
    fn concurrent_compare_updates_do_not_lose_increments() {
        let temp = tempfile::tempdir().unwrap();
        let path = std::sync::Arc::new(temp.path().join("state.json"));
        let workers = (0..4)
            .map(|_| {
                let path = path.clone();
                std::thread::spawn(move || {
                    for _ in 0..20 {
                        loop {
                            let current = {
                                let _guard = mutex().lock().unwrap();
                                load(&path).unwrap().get("count").cloned()
                            };
                            let next = Value::from(
                                current.as_ref().and_then(Value::as_u64).unwrap_or(0) + 1,
                            );
                            let _guard = mutex().lock().unwrap();
                            if compare_at(&path, "count", current.as_ref(), Some(next)).unwrap() {
                                break;
                            }
                        }
                    }
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            worker.join().unwrap();
        }
        assert_eq!(load(&path).unwrap()["count"], Value::from(80));
    }
}

pub use super::credentials::{
    credential_backend, secret_available, secret_delete, secret_get, secret_response, secret_set,
    set_credential_backend,
};
