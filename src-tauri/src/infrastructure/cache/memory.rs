//! Bounded generic in-process cache for expensive derived presentation results.
use std::{collections::{HashMap, VecDeque}, hash::Hash, time::{Duration, Instant}};
pub struct MemoryCache<K, V> {
    values: HashMap<K, (V, Instant)>,
    order: VecDeque<K>,
    limit: usize,
    ttl: Duration,
}
impl<K: Clone + Eq + Hash, V> MemoryCache<K, V> {
    pub fn new(limit: usize, ttl: Duration) -> Self {
        Self { values: HashMap::new(), order: VecDeque::new(), limit: limit.max(1), ttl }
    }
    pub fn get(&mut self, key: &K) -> Option<&V> {
        if self.values.get(key).is_some_and(|(_, at)| at.elapsed() > self.ttl) {
            self.values.remove(key);
            self.order.retain(|item| item != key);
        }
        if self.values.contains_key(key) {
            self.order.retain(|item| item != key);
            self.order.push_back(key.clone());
        }
        self.values.get(key).map(|(value, _)| value)
    }
    pub fn put(&mut self, key: K, value: V) {
        self.order.retain(|item| item != &key);
        self.order.push_back(key.clone());
        self.values.insert(key, (value, Instant::now()));
        while self.values.len() > self.limit {
            if let Some(old) = self.order.pop_front() { self.values.remove(&old); }
        }
    }
    pub fn invalidate(&mut self, key: &K) {
        self.values.remove(key);
        self.order.retain(|item| item != key);
    }
    pub fn clear(&mut self) { self.values.clear(); self.order.clear(); }
}
