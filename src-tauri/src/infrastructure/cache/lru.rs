//! Hash map backed LRU with constant-time promotion and eviction.
use std::{borrow::Borrow, collections::HashMap, hash::Hash};

struct Node<K, V> {
    value: V,
    previous: Option<K>,
    next: Option<K>,
}

pub(crate) struct LruMap<K, V> {
    entries: HashMap<K, Node<K, V>>,
    least_recent: Option<K>,
    most_recent: Option<K>,
}

impl<K, V> Default for LruMap<K, V> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            least_recent: None,
            most_recent: None,
        }
    }
}

impl<K: Eq + Hash + Clone, V> LruMap<K, V> {
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        let key = self.entries.get_key_value(key)?.0.clone();
        self.promote(&key);
        self.entries.get_mut::<K>(&key).map(|node| &mut node.value)
    }

    pub(crate) fn insert(&mut self, key: K, value: V) -> Option<V> {
        if let Some(node) = self.entries.get_mut(&key) {
            let old = std::mem::replace(&mut node.value, value);
            self.promote(&key);
            return Some(old);
        }
        let previous = self.most_recent.clone();
        let _ = self.entries.insert(
            key.clone(),
            Node {
                value,
                previous: previous.clone(),
                next: None,
            },
        );
        if let Some(previous) = previous {
            if let Some(node) = self.entries.get_mut(&previous) {
                node.next = Some(key.clone());
            }
        } else {
            self.least_recent = Some(key.clone());
        }
        self.most_recent = Some(key);
        None
    }

    pub(crate) fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        let key = self.entries.get_key_value(key)?.0.clone();
        let node = self.entries.remove::<K>(&key)?;
        self.unlink(&key, node.previous.as_ref(), node.next.as_ref());
        Some(node.value)
    }

    pub(crate) fn pop_lru(&mut self) -> Option<(K, V)> {
        let key = self.least_recent.clone()?;
        self.remove(&key).map(|value| (key, value))
    }

    pub(crate) fn pop_lru_where(
        &mut self,
        mut matches: impl FnMut(&K, &V) -> bool,
    ) -> Option<(K, V)> {
        let mut current = self.least_recent.clone();
        while let Some(key) = current {
            let (matched, next) = {
                let node = self.entries.get(&key)?;
                (matches(&key, &node.value), node.next.clone())
            };
            if matched {
                return self.remove(&key).map(|value| (key, value));
            }
            current = next;
        }
        None
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.least_recent = None;
        self.most_recent = None;
    }

    fn promote(&mut self, key: &K) {
        if self.most_recent.as_ref() == Some(key) {
            return;
        }
        let Some((previous, next)) = self
            .entries
            .get(key)
            .map(|node| (node.previous.clone(), node.next.clone()))
        else {
            return;
        };
        self.unlink(key, previous.as_ref(), next.as_ref());

        let previous = self.most_recent.replace(key.clone());
        if let Some(previous) = previous.as_ref() {
            if let Some(node) = self.entries.get_mut(previous) {
                node.next = Some(key.clone());
            }
        } else {
            self.least_recent = Some(key.clone());
        }
        if let Some(node) = self.entries.get_mut(key) {
            node.previous = previous;
            node.next = None;
        }
    }

    fn unlink(&mut self, key: &K, previous: Option<&K>, next: Option<&K>) {
        if let Some(previous) = previous {
            if let Some(node) = self.entries.get_mut(previous) {
                node.next = next.cloned();
            }
        } else if self.least_recent.as_ref() == Some(key) {
            self.least_recent = next.cloned();
        }

        if let Some(next) = next {
            if let Some(node) = self.entries.get_mut(next) {
                node.previous = previous.cloned();
            }
        } else if self.most_recent.as_ref() == Some(key) {
            self.most_recent = previous.cloned();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LruMap;

    #[test]
    fn promotes_on_read_and_evicts_least_recent() {
        let mut values = LruMap::default();
        let _ = values.insert("a", 1);
        let _ = values.insert("b", 2);
        assert_eq!(*values.get_mut(&"a").unwrap(), 1);
        let _ = values.insert("c", 3);
        assert_eq!(values.pop_lru(), Some(("b", 2)));
        assert_eq!(values.pop_lru(), Some(("a", 1)));
        assert_eq!(values.pop_lru(), Some(("c", 3)));
        assert_eq!(values.len(), 0);
    }

    #[test]
    fn replacing_and_removing_entries_keep_links_consistent() {
        let mut values = LruMap::default();
        let _ = values.insert(1, "one");
        let _ = values.insert(2, "two");
        assert_eq!(values.insert(1, "updated"), Some("one"));
        assert_eq!(values.remove(&2), Some("two"));
        assert_eq!(values.pop_lru(), Some((1, "updated")));
        assert!(values.pop_lru().is_none());
    }
}
