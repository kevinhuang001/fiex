//! Small, revision-keyed cache for Core-generated presentation data.
//! Plugin responses and permission-scoped sessions must always be resolved anew.
use serde_json::Value;
use std::{
    collections::HashMap,
    path::Path,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant, UNIX_EPOCH},
};

const TTL: Duration = Duration::from_secs(30);
const MAX_ENTRIES: usize = 32;
const MAX_BYTES: usize = 2 * 1024 * 1024;

struct Entry {
    value: Value,
    inserted: Instant,
    bytes: usize,
}

#[derive(Default)]
struct Cache {
    entries: HashMap<String, Entry>,
    bytes: usize,
}

fn cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(Cache::default()))
}

/// Include the file revision and locale in keys. No entry is retained if stat fails.
pub fn key(kind: &str, path: &Path) -> Option<String> {
    let stat = path.metadata().ok()?;
    let modified = stat.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some(format!(
        "{kind}:{}:{}:{}:{}:{}",
        crate::i18n::locale(),
        path.display(),
        stat.len(),
        modified.as_secs(),
        modified.subsec_nanos()
    ))
}

pub fn get(key: &str) -> Option<Value> {
    let mut cache = cache().lock().ok()?;
    let entry = cache.entries.get(key)?;
    if entry.inserted.elapsed() >= TTL {
        let removed = cache.entries.remove(key)?;
        cache.bytes -= removed.bytes;
        return None;
    }
    Some(entry.value.clone())
}

pub fn put(key: String, value: Value) {
    let Ok(bytes) = serde_json::to_vec(&value).map(|value| value.len()) else {
        return;
    };
    if bytes > MAX_BYTES / 2 {
        return;
    }
    let Ok(mut cache) = cache().lock() else {
        return;
    };
    if let Some(old) = cache.entries.remove(&key) {
        cache.bytes -= old.bytes;
    }
    cache.entries.retain(|_, entry| entry.inserted.elapsed() < TTL);
    cache.bytes = cache.entries.values().map(|entry| entry.bytes).sum();
    while cache.entries.len() >= MAX_ENTRIES || cache.bytes + bytes > MAX_BYTES {
        let Some(oldest) = cache
            .entries
            .iter()
            .min_by_key(|(_, entry)| entry.inserted)
            .map(|(key, _)| key.clone())
        else {
            break;
        };
        if let Some(old) = cache.entries.remove(&oldest) {
            cache.bytes -= old.bytes;
        }
    }
    cache.bytes += bytes;
    cache.entries.insert(
        key,
        Entry {
            value,
            inserted: Instant::now(),
            bytes,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_revision_changes_key() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("file.txt");
        std::fs::write(&file, b"a").unwrap();
        let before = key("text", &file).unwrap();
        std::fs::write(&file, b"longer").unwrap();
        assert_ne!(before, key("text", &file).unwrap());
    }

    #[test]
    fn cached_values_are_bounded() {
        let key = "cache-test".to_string();
        put(key.clone(), serde_json::json!({"ok":true}));
        assert_eq!(get(&key), Some(serde_json::json!({"ok":true})));
        put("oversize".into(), Value::String("x".repeat(MAX_BYTES)));
        assert!(get("oversize").is_none());
    }
}
