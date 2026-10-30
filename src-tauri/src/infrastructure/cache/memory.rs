//! In-process, size-bounded LRU with TTL. Loaders and authorization live
//! outside this generic backend; no provider-specific keys or logic here.
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};
struct Slot<V> {
    value: V,
    expires: Instant,
    access: u64,
}
pub struct MemoryCache<V> {
    capacity: usize,
    ttl: Duration,
    clock: AtomicU64,
    entries: Mutex<HashMap<String, Slot<V>>>,
}
impl<V: Clone> MemoryCache<V> {
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        Self {
            capacity: capacity.max(1),
            ttl,
            clock: AtomicU64::new(1),
            entries: Mutex::new(HashMap::new()),
        }
    }
    pub fn get(&self, key: &str) -> Option<V> {
        let mut entries = self.entries.lock().ok()?;
        let entry = entries.get_mut(key)?;
        if Instant::now() >= entry.expires {
            entries.remove(key);
            return None;
        }
        entry.access = self.clock.fetch_add(1, Ordering::Relaxed);
        Some(entry.value.clone())
    }
    pub fn insert(&self, key: String, value: V) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.retain(|_, entry| Instant::now() < entry.expires);
            if !entries.contains_key(&key) && entries.len() >= self.capacity {
                if let Some(victim) = entries
                    .iter()
                    .min_by_key(|(_, entry)| entry.access)
                    .map(|(key, _)| key.clone())
                {
                    entries.remove(&victim);
                }
            }
            entries.insert(
                key,
                Slot {
                    value,
                    expires: Instant::now() + self.ttl,
                    access: self.clock.fetch_add(1, Ordering::Relaxed),
                },
            );
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
