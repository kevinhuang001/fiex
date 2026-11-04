//! In-process, size-bounded LRU with TTL. Loaders and authorization live
//! outside this generic backend; no provider-specific keys or logic here.
use super::lru::LruMap;
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};
struct Slot<V> {
    value: V,
    expires: Instant,
}
pub struct MemoryCache<V> {
    capacity: usize,
    ttl: Duration,
    entries: Mutex<LruMap<String, Slot<V>>>,
}
impl<V: Clone> MemoryCache<V> {
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        Self {
            capacity: capacity.max(1),
            ttl,
            entries: Mutex::new(LruMap::default()),
        }
    }
    pub fn get(&self, key: &str) -> Option<V> {
        let mut entries = self.entries.lock().ok()?;
        let value = {
            let entry = entries.get_mut(key)?;
            if Instant::now() >= entry.expires {
                None
            } else {
                Some(entry.value.clone())
            }
        };
        if value.is_none() {
            let _ = entries.remove(key);
        }
        value
    }
    pub fn insert(&self, key: String, value: V) {
        if let Ok(mut entries) = self.entries.lock() {
            let _ = entries.insert(
                key,
                Slot {
                    value,
                    expires: Instant::now() + self.ttl,
                },
            );
            while entries.len() > self.capacity {
                let _ = entries.pop_lru();
            }
        }
    }
    pub fn invalidate(&self, key: &str) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.remove(key);
        }
    }
    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.clear();
        }
    }
}
