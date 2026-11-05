//! Single persistent engine shared by Native Core Modules and authenticated
//! Plugin Modules. Callers supply a trusted OwnerId, never a wire value.
#[cfg(test)]
use super::owner::CoreOwnerKey;
use super::owner::OwnerId;

mod layout;
pub(super) use layout::validate;
use layout::{file, max_value_bytes, valid_segment};
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
mod tests;
