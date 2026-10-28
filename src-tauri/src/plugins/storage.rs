//! Host-owned per-plugin persistent storage and credential broker.
//! No plugin-controlled absolute paths; all write operations are quota limited.
//! APIs exposed to plugins MUST be routed through a verified session identity.
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
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
fn validate(plugin: &str, scope: &str, key: &str) -> Result<(), String> {
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
/// Whether this process can reach an OS credential store. A desktop with a
/// Secret Service, Keychain or Credential Manager answers `true`; WSL and
/// headless Linux sessions answer `false` and use the local fallback below.
fn keyring_ready() -> bool {
    static READY: OnceLock<bool> = OnceLock::new();
    *READY.get_or_init(|| {
        let ready = keyring::v1::Entry::store_status().is_ok();
        if !ready {
            if let Ok(path) = fallback_file() {
                eprintln!(
                    "fiex: no OS credential store; plugin secrets are kept in {} (0600)",
                    path.display()
                );
            }
        }
        ready
    })
}

fn fallback_file() -> Result<PathBuf, String> {
    dirs::data_local_dir()
        .ok_or_else(|| "application data directory unavailable".to_string())
        .map(|data| data.join("fiex").join("secrets.json"))
}

fn fallback_key(plugin: &str, scope: &str, key: &str) -> String {
    format!("{plugin}:{scope}:{key}")
}

fn load_fallback(path: &Path) -> Result<BTreeMap<String, String>, String> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|error| format!("secret store unreadable: {error}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(error) => Err(error.to_string()),
    }
}

/// Atomic 0600 replacement: secrets never land in the registry file and a
/// crash leaves either the old or the new store, never a partial one.
fn save_fallback(path: &Path, items: &BTreeMap<String, String>) -> Result<(), String> {
    let parent = path.parent().ok_or("invalid secret storage path")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec(items).map_err(|error| error.to_string())?;
    let staging = parent.join(format!("secrets-{}.tmp", std::process::id()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&staging)
            .map_err(|error| error.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(|error| error.to_string())?;
        }
        file.write_all(&bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        #[cfg(windows)]
        if path.exists() {
            fs::remove_file(path).map_err(|error| error.to_string())?;
        }
        fs::rename(&staging, path).map_err(|error| error.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&staging);
    }
    result
}

fn fallback_set(plugin: &str, scope: &str, key: &str, value: &str) -> Result<(), String> {
    let _guard = mutex().lock().map_err(|_| "secret store lock poisoned")?;
    let path = fallback_file()?;
    let mut items = load_fallback(&path)?;
    items.insert(fallback_key(plugin, scope, key), value.to_string());
    save_fallback(&path, &items)
}

fn fallback_get(plugin: &str, scope: &str, key: &str) -> Result<String, String> {
    let _guard = mutex().lock().map_err(|_| "secret store lock poisoned")?;
    load_fallback(&fallback_file()?)?
        .remove(&fallback_key(plugin, scope, key))
        .ok_or_else(|| "no matching credential".to_string())
}

fn fallback_delete(plugin: &str, scope: &str, key: &str) -> Result<(), String> {
    let _guard = mutex().lock().map_err(|_| "secret store lock poisoned")?;
    let path = fallback_file()?;
    let mut items = load_fallback(&path)?;
    if items.remove(&fallback_key(plugin, scope, key)).is_some() {
        save_fallback(&path, &items)?;
    }
    Ok(())
}

fn secret_entry(plugin: &str, scope: &str, key: &str) -> Result<keyring::v1::Entry, String> {
    validate(plugin, scope, key)?;
    keyring::v1::Entry::new("org.fiex.plugin", &fallback_key(plugin, scope, key))
        .map_err(|e| format!("system credential store unavailable: {e}"))
}
pub fn secret_set(plugin: &str, scope: &str, key: &str, value: &str) -> Result<(), String> {
    validate(plugin, scope, key)?;
    if value.len() > 65536 {
        return Err("secret too large".into());
    }
    if !keyring_ready() {
        return fallback_set(plugin, scope, key, value);
    }
    secret_entry(plugin, scope, key)?
        .set_password(value)
        .map_err(|e| format!("credential store write failed: {e}"))
}
/// Called only by authenticated host IPC; NEVER expose to untrusted iframe Tauri invoke.
pub fn secret_get(plugin: &str, scope: &str, key: &str) -> Result<String, String> {
    validate(plugin, scope, key)?;
    if !keyring_ready() {
        return fallback_get(plugin, scope, key);
    }
    secret_entry(plugin, scope, key)?
        .get_password()
        .map_err(|e| format!("credential store access failed: {e}"))
}
pub fn secret_delete(plugin: &str, scope: &str, key: &str) -> Result<(), String> {
    validate(plugin, scope, key)?;
    if !keyring_ready() {
        return fallback_delete(plugin, scope, key);
    }
    secret_entry(plugin, scope, key)?
        .delete_credential()
        .map_err(|e| format!("credential store delete failed: {e}"))
}
pub fn secret_available() -> bool {
    keyring_ready() || fallback_file().is_ok()
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

/// Typed wire status for the authenticated SDK keychain service. Access denied
/// includes a locked store or a platform/user authorization refusal; it is not
/// conflated with an absent credential or an unavailable backend.
pub fn secret_response(
    plugin: &str,
    scope: &str,
    key: &str,
    operation: &str,
    value: Option<&str>,
) -> Result<Value, String> {
    use keyring::v1::{Entry, Error};
    use serde_json::json;
    fn error(error: Error) -> Value {
        let status = match &error {
            Error::NoEntry => "missing",
            Error::NoStorageAccess(_) => "denied",
            _ => "unavailable",
        };
        json!({"status":status,"message":error.to_string()})
    }
    validate(plugin, scope, key)?;
    // Without an OS store the same typed statuses come from the local fallback,
    // so plugins cannot tell the two paths apart except through `available`.
    if !keyring_ready() {
        return Ok(match operation {
            "get" => match fallback_get(plugin, scope, key) {
                Ok(value) => json!({"status":"present","value":value}),
                Err(_) => json!({"status":"missing"}),
            },
            "set" => {
                let value = value.ok_or("missing secret value")?;
                if value.len() > 65536 {
                    return Err("secret too large".into());
                }
                fallback_set(plugin, scope, key, value)?;
                json!({"status":"ok"})
            }
            "delete" => {
                fallback_delete(plugin, scope, key)?;
                json!({"status":"ok"})
            }
            _ => return Err("unknown credential operation".into()),
        });
    }
    let entry = match Entry::new("org.fiex.plugin", &fallback_key(plugin, scope, key)) {
        Ok(entry) => entry,
        Err(failure) => return Ok(error(failure)),
    };
    Ok(match operation {
        "get" => match entry.get_password() {
            Ok(value) => json!({"status":"present","value":value}),
            Err(failure) => error(failure),
        },
        "set" => {
            let value = value.ok_or("missing secret value")?;
            if value.len() > 65536 {
                return Err("secret too large".into());
            }
            match entry.set_password(value) {
                Ok(()) => json!({"status":"ok"}),
                Err(failure) => error(failure),
            }
        }
        "delete" => match entry.delete_credential() {
            Ok(()) | Err(Error::NoEntry) => json!({"status":"ok"}),
            Err(failure) => error(failure),
        },
        _ => return Err("unknown credential operation".into()),
    })
}

#[cfg(test)]
mod secret_fallback_tests {
    use super::*;

    #[test]
    fn fallback_round_trips_sets_and_deletes() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("secrets.json");
        let mut items = load_fallback(&path).unwrap();
        assert!(items.is_empty());
        items.insert(
            fallback_key("p.one", "connection-a", "password"),
            "s3cret".into(),
        );
        save_fallback(&path, &items).unwrap();
        assert_eq!(
            load_fallback(&path)
                .unwrap()
                .remove(&fallback_key("p.one", "connection-a", "password"))
                .unwrap(),
            "s3cret"
        );
        // Keys are scoped: another plugin, scope or field never matches.
        let store = load_fallback(&path).unwrap();
        assert!(!store.contains_key(&fallback_key("p.two", "connection-a", "password")));
        assert!(!store.contains_key(&fallback_key("p.one", "connection-b", "password")));
        assert!(!store.contains_key(&fallback_key("p.one", "connection-a", "username")));
        let mut empty = BTreeMap::new();
        save_fallback(&path, &mut empty).unwrap();
        assert!(load_fallback(&path).unwrap().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn fallback_file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("secrets.json");
        let mut items = BTreeMap::new();
        items.insert(fallback_key("p", "s", "k"), "v".into());
        save_fallback(&path, &items).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "{mode:o}");
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
