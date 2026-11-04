//! Single persistent engine shared by Native Core Modules and authenticated
//! Plugin Modules. Callers supply a trusted OwnerId, never a wire value.
use super::owner::{CoreOwnerKey, OwnerId};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};

const MAX_VALUE_BYTES: usize = 1024 * 1024;
const MAX_SCOPE_BYTES: usize = 16 * 1024 * 1024;
const MAX_CACHED_SCOPES: usize = 128;

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

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct StorageKey {
    owner: OwnerId,
    scope: String,
}

struct Snapshot {
    path: PathBuf,
    items: Option<BTreeMap<String, Value>>,
}

impl Snapshot {
    fn new(path: PathBuf) -> Self {
        Self { path, items: None }
    }

    fn items(&mut self) -> Result<&mut BTreeMap<String, Value>, String> {
        if self.items.is_none() {
            self.items = Some(load(&self.path)?);
        }
        Ok(self.items.as_mut().expect("snapshot was loaded"))
    }

    fn persist(&mut self) -> Result<(), String> {
        let result = save(
            &self.path,
            self.items.as_ref().expect("snapshot was loaded"),
        );
        if result.is_err() {
            // Persistence may fail before or just after atomic replacement.
            // Reload disk on the next operation instead of serving a stale cache.
            self.items = None;
        }
        result
    }

    fn get(&mut self, key: &str) -> Result<Option<Value>, String> {
        Ok(self.items()?.get(key).cloned())
    }

    fn set(&mut self, key: &str, value: Value) -> Result<(), String> {
        if self.items()?.get(key) == Some(&value) {
            return Ok(());
        }
        self.items()?.insert(key.to_owned(), value);
        self.persist()
    }

    fn delete(&mut self, key: &str) -> Result<(), String> {
        if self.items()?.remove(key).is_none() {
            return Ok(());
        }
        self.persist()
    }

    fn list_keys(&mut self) -> Result<Vec<String>, String> {
        Ok(self.items()?.keys().cloned().collect())
    }

    fn compare_and_set(
        &mut self,
        key: &str,
        expected: Option<&Value>,
        next: Option<Value>,
    ) -> Result<bool, String> {
        if self.items()?.get(key) != expected {
            return Ok(false);
        }
        let items = self.items()?;
        match next {
            Some(value) if items.get(key) != Some(&value) => {
                items.insert(key.to_owned(), value);
            }
            None if items.contains_key(key) => {
                items.remove(key);
            }
            _ => return Ok(true),
        }
        self.persist()?;
        Ok(true)
    }
}

struct CacheEntry {
    snapshot: Arc<Mutex<Snapshot>>,
    last_used: u64,
}

#[derive(Default)]
struct ScopeCache {
    entries: HashMap<StorageKey, CacheEntry>,
    clock: u64,
}

impl ScopeCache {
    fn get_or_create(
        &mut self,
        key: StorageKey,
        path: PathBuf,
    ) -> Result<Arc<Mutex<Snapshot>>, String> {
        self.clock = self.clock.wrapping_add(1);
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.last_used = self.clock;
            return Ok(Arc::clone(&entry.snapshot));
        }

        if self.entries.len() == MAX_CACHED_SCOPES {
            let mut candidates = self
                .entries
                .iter()
                .filter(|(_, entry)| Arc::strong_count(&entry.snapshot) == 1)
                .map(|(key, entry)| (key.clone(), entry.last_used))
                .collect::<Vec<_>>();
            if let Some((oldest, _)) = candidates.drain(..).min_by_key(|(_, last_used)| *last_used)
            {
                self.entries.remove(&oldest);
            } else {
                // All cached scopes are actively used. Keep the bound by waiting
                // for the least-recently-used scope to become idle before evicting it.
                let (oldest, entry) = self
                    .entries
                    .iter()
                    .min_by_key(|(_, entry)| entry.last_used)
                    .expect("a full cache has entries");
                let oldest = oldest.clone();
                let snapshot = Arc::clone(&entry.snapshot);
                loop {
                    let guard = snapshot
                        .lock()
                        .map_err(|_| "plugin storage lock poisoned")?;
                    if Arc::strong_count(&snapshot) == 2 {
                        drop(guard);
                        self.entries.remove(&oldest);
                        break;
                    }
                    drop(guard);
                    std::thread::yield_now();
                }
            }
        }

        let snapshot = Arc::new(Mutex::new(Snapshot::new(path)));
        self.entries.insert(
            key,
            CacheEntry {
                snapshot: Arc::clone(&snapshot),
                last_used: self.clock,
            },
        );
        Ok(snapshot)
    }
}

fn cache() -> &'static Mutex<ScopeCache> {
    static CACHE: OnceLock<Mutex<ScopeCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(ScopeCache::default()))
}

fn with_scope<T>(
    owner: &OwnerId,
    scope: &str,
    operation: impl FnOnce(&mut Snapshot) -> Result<T, String>,
) -> Result<T, String> {
    let key = StorageKey {
        owner: owner.clone(),
        scope: scope.to_owned(),
    };
    let path = file(owner, scope)?;
    let cached = cache()
        .lock()
        .map_err(|_| "plugin storage cache lock poisoned")?
        .get_or_create(key, path.clone())?;
    let mut snapshot = cached.lock().map_err(|_| "plugin storage lock poisoned")?;
    operation(&mut snapshot)
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
    #[cfg(unix)]
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())?;
    Ok(())
}

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
    with_scope(owner, scope, |snapshot| {
        snapshot.compare_and_set(key, expected.as_ref(), next)
    })
}

pub(super) fn get(owner: &OwnerId, scope: &str, key: &str) -> Result<Option<Value>, String> {
    validate(owner, scope, key)?;
    with_scope(owner, scope, |snapshot| snapshot.get(key))
}

pub(super) fn set(owner: &OwnerId, scope: &str, key: &str, value: Value) -> Result<(), String> {
    validate(owner, scope, key)?;
    if serde_json::to_vec(&value).map_err(|e| e.to_string())?.len()
        > max_value_bytes(owner, scope, key)
    {
        return Err("plugin storage value too large".into());
    }
    with_scope(owner, scope, |snapshot| snapshot.set(key, value))
}

pub(super) fn delete(owner: &OwnerId, scope: &str, key: &str) -> Result<(), String> {
    validate(owner, scope, key)?;
    with_scope(owner, scope, |snapshot| snapshot.delete(key))
}

pub(super) fn list_keys(owner: &OwnerId, scope: &str) -> Result<Vec<String>, String> {
    if !owner.is_valid() || !valid_segment(scope) {
        return Err("invalid storage owner or scope".into());
    }
    with_scope(owner, scope, Snapshot::list_keys)
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

    #[test]
    fn cas_distinguishes_absent_null_and_conflicts_and_skips_noop_saves() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("state.json");
        let mut state = Snapshot::new(path.clone());
        assert!(state
            .compare_and_set("key", None, Some(Value::Null))
            .unwrap());
        assert!(!state
            .compare_and_set("key", None, Some(Value::Bool(true)))
            .unwrap());
        assert!(state
            .compare_and_set("key", Some(&Value::Null), Some(Value::from(1)))
            .unwrap());
        assert!(!state
            .compare_and_set("key", Some(&Value::Null), None)
            .unwrap());
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        assert!(state
            .compare_and_set("key", Some(&Value::from(1)), Some(Value::from(1)))
            .unwrap());
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
        assert_eq!(state.get("key").unwrap(), Some(Value::from(1)));
    }

    #[test]
    fn set_equal_and_delete_missing_do_not_write() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("state.json");
        let mut state = Snapshot::new(path.clone());
        state.set("key", Value::from(1)).unwrap();
        let before = fs::metadata(&path).unwrap().modified().unwrap();
        state.set("key", Value::from(1)).unwrap();
        state.delete("absent").unwrap();
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), before);
    }

    #[test]
    fn scope_cache_is_bounded_and_keeps_scope_locks_separate() {
        let mut cache = ScopeCache::default();
        for index in 0..(MAX_CACHED_SCOPES + 1) {
            drop(
                cache
                    .get_or_create(
                        StorageKey {
                            owner: OwnerId::plugin("valid.plugin"),
                            scope: format!("scope{index}"),
                        },
                        PathBuf::from(format!("/tmp/fiex-storage-{index}.json")),
                    )
                    .unwrap(),
            );
        }
        assert_eq!(cache.entries.len(), MAX_CACHED_SCOPES);
        let first = cache
            .get_or_create(
                StorageKey {
                    owner: OwnerId::plugin("valid.plugin"),
                    scope: "one".into(),
                },
                PathBuf::from("/tmp/fiex-storage-one.json"),
            )
            .unwrap();
        let second = cache
            .get_or_create(
                StorageKey {
                    owner: OwnerId::plugin("valid.plugin"),
                    scope: "two".into(),
                },
                PathBuf::from("/tmp/fiex-storage-two.json"),
            )
            .unwrap();
        let _guard = first.lock().unwrap();
        assert!(second.try_lock().is_ok());
    }

    #[test]
    fn concurrent_compare_updates_do_not_lose_increments() {
        let temp = tempfile::tempdir().unwrap();
        let snapshot = Arc::new(Mutex::new(Snapshot::new(temp.path().join("state.json"))));
        let workers = (0..4)
            .map(|_| {
                let snapshot = Arc::clone(&snapshot);
                std::thread::spawn(move || {
                    for _ in 0..20 {
                        loop {
                            let current = snapshot.lock().unwrap().get("count").unwrap();
                            let next = Value::from(
                                current.as_ref().and_then(Value::as_u64).unwrap_or(0) + 1,
                            );
                            if snapshot
                                .lock()
                                .unwrap()
                                .compare_and_set("count", current.as_ref(), Some(next))
                                .unwrap()
                            {
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
        assert_eq!(
            snapshot.lock().unwrap().get("count").unwrap(),
            Some(Value::from(80))
        );
    }
}
