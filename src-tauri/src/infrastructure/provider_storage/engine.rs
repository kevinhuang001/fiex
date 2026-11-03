//! Single persistent engine shared by Native Core Modules and authenticated
//! Plugin Modules. Callers supply a trusted OwnerId, never a wire value.
use super::owner::{CoreOwnerKey, OwnerId};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

const MAX_VALUE_BYTES: usize = 1024 * 1024;
const MAX_SCOPE_BYTES: usize = 16 * 1024 * 1024;

fn max_value_bytes(owner: &OwnerId, scope: &str, key: &str) -> usize {
    if owner.is_core(CoreOwnerKey::new("applications")) && scope == "tree" && key == "state" {
        MAX_SCOPE_BYTES - 1024
    } else {
        MAX_VALUE_BYTES
    }
}

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
pub(super) fn validate(owner: &OwnerId, scope: &str, key: &str) -> Result<(), String> {
    if !owner.is_valid() || !valid_segment(scope) || !valid_segment(key) {
        return Err("invalid storage owner, scope or key".into());
    }
    Ok(())
}
fn base() -> Result<PathBuf, String> {
    dirs::data_local_dir()
        .ok_or("application data directory unavailable".to_string())
        .map(|d| d.join("fiex").join("plugin-state"))
}
fn file(owner: &OwnerId, scope: &str) -> Result<PathBuf, String> {
    if !owner.is_valid() || !valid_segment(scope) {
        return Err("invalid storage owner or scope".into());
    }
    let directory = match owner {
        OwnerId::Plugin(plugin) => base()?.join(plugin),
        OwnerId::Core(key) => base()?.join("core").join(key.as_str()),
    };
    Ok(directory.join(format!("{scope}.json")))
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
pub(super) fn compare_and_set(
    owner: &OwnerId,
    scope: &str,
    key: &str,
    expected: Option<Value>,
    next: Option<Value>,
) -> Result<bool, String> {
    validate(owner, scope, key)?;
    if let Some(value) = &next {
        if serde_json::to_vec(value)
            .map_err(|error| error.to_string())?
            .len()
            > max_value_bytes(owner, scope, key)
        {
            return Err("plugin storage value too large".into());
        }
    }
    let _guard = mutex().lock().map_err(|_| "plugin storage lock poisoned")?;
    compare_at(&file(owner, scope)?, key, expected.as_ref(), next)
}

pub(super) fn get(owner: &OwnerId, scope: &str, key: &str) -> Result<Option<Value>, String> {
    validate(owner, scope, key)?;
    let _guard = mutex().lock().map_err(|_| "plugin storage lock poisoned")?;
    Ok(load(&file(owner, scope)?)?.remove(key))
}
pub(super) fn set(owner: &OwnerId, scope: &str, key: &str, value: Value) -> Result<(), String> {
    validate(owner, scope, key)?;
    if serde_json::to_vec(&value).map_err(|e| e.to_string())?.len()
        > max_value_bytes(owner, scope, key)
    {
        return Err("plugin storage value too large".into());
    }
    let _guard = mutex().lock().map_err(|_| "plugin storage lock poisoned")?;
    let path = file(owner, scope)?;
    let mut items = load(&path)?;
    items.insert(key.into(), value);
    save(&path, &items)
}
pub(super) fn delete(owner: &OwnerId, scope: &str, key: &str) -> Result<(), String> {
    validate(owner, scope, key)?;
    let _guard = mutex().lock().map_err(|_| "plugin storage lock poisoned")?;
    let path = file(owner, scope)?;
    let mut items = load(&path)?;
    items.remove(key);
    save(&path, &items)
}
pub(super) fn list_keys(owner: &OwnerId, scope: &str) -> Result<Vec<String>, String> {
    if !owner.is_valid() || !valid_segment(scope) {
        return Err("invalid storage owner or scope".into());
    }
    let _guard = mutex().lock().map_err(|_| "plugin storage lock poisoned")?;
    Ok(load(&file(owner, scope)?)?.into_keys().collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_traversal() {
        let owner = OwnerId::plugin("valid.plugin");
        assert!(set(
            &OwnerId::Plugin("../evil".into()),
            "global",
            "safe",
            Value::Null
        )
        .is_err());
        assert!(set(&owner, "../x", "safe", Value::Null).is_err());
        assert!(set(&owner, "global", "../x", Value::Null).is_err());
    }
    #[test]
    fn size_cap() {
        let owner = OwnerId::plugin("valid.plugin");
        assert!(set(
            &owner,
            "global",
            "test",
            Value::String("a".repeat(MAX_VALUE_BYTES + 1))
        )
        .is_err());
    }

    #[test]
    fn applications_tree_can_use_atomic_scope_quota_without_raising_plugin_limit() {
        let applications = OwnerId::core(CoreOwnerKey::new("applications"));
        let plugin = OwnerId::plugin("valid.plugin");
        assert_eq!(
            max_value_bytes(&applications, "tree", "state"),
            MAX_SCOPE_BYTES - 1024
        );
        assert_eq!(max_value_bytes(&plugin, "tree", "state"), MAX_VALUE_BYTES);
        assert_eq!(
            max_value_bytes(&applications, "other", "state"),
            MAX_VALUE_BYTES
        );
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
