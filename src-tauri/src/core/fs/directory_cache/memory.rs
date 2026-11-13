use super::DirectorySnapshot;
use crate::infrastructure::cache::{disk, lru::LruMap};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex, OnceLock},
};

const MAX_DIRECTORIES: usize = 10_000;
const MAX_ENTRIES: usize = 500_000;

#[derive(Default)]
struct DirectoryCache {
    values: LruMap<String, Arc<DirectorySnapshot>>,
    entry_count: usize,
}

impl DirectoryCache {
    fn get(&mut self, identity: &str) -> Option<Arc<DirectorySnapshot>> {
        self.values.get_mut(identity).cloned()
    }

    fn remember(&mut self, identity: &str, data: Arc<DirectorySnapshot>) {
        if let Some(previous) = self.values.remove(identity) {
            self.entry_count = self
                .entry_count
                .saturating_sub(previous.listing.entries.len());
        }
        if data.listing.entries.len() > MAX_ENTRIES {
            return;
        }
        while self.values.len() >= MAX_DIRECTORIES
            || self.entry_count.saturating_add(data.listing.entries.len()) > MAX_ENTRIES
        {
            let Some((_, previous)) = self.values.pop_lru() else {
                break;
            };
            self.entry_count = self
                .entry_count
                .saturating_sub(previous.listing.entries.len());
        }
        self.entry_count = self.entry_count.saturating_add(data.listing.entries.len());
        let _ = self.values.insert(identity.to_owned(), data);
    }

    fn mutate(&mut self, identity: &str, update: impl FnOnce(&mut DirectorySnapshot)) {
        let (previous_count, next_count) = match self.values.get_mut(identity) {
            Some(snapshot) => {
                let snapshot = Arc::make_mut(snapshot);
                let previous_count = snapshot.listing.entries.len();
                update(snapshot);
                (previous_count, snapshot.listing.entries.len())
            }
            None => return,
        };
        self.entry_count = self
            .entry_count
            .saturating_sub(previous_count)
            .saturating_add(next_count);
        if next_count > MAX_ENTRIES {
            self.remove(identity);
        } else {
            while self.entry_count > MAX_ENTRIES {
                let Some((_, removed)) = self.values.pop_lru() else {
                    break;
                };
                self.entry_count = self
                    .entry_count
                    .saturating_sub(removed.listing.entries.len());
            }
        }
    }

    fn remove(&mut self, identity: &str) {
        if let Some(snapshot) = self.values.remove(identity) {
            self.entry_count = self
                .entry_count
                .saturating_sub(snapshot.listing.entries.len());
        }
    }

    fn remove_many(&mut self, identities: &HashSet<String>) {
        for identity in identities {
            self.remove(identity);
        }
    }
}

fn cache() -> &'static Mutex<DirectoryCache> {
    static CACHE: OnceLock<Mutex<DirectoryCache>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

pub(super) fn snapshot(identity: &str) -> Option<DirectorySnapshot> {
    let memory = cache()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get(identity);
    if let Some(data) = memory {
        return Some(data.as_ref().clone());
    }
    let data = disk::load::<DirectorySnapshot>(identity)?;
    remember(identity, Arc::new(data.clone()));
    Some(data)
}

pub(super) fn remember(identity: &str, data: Arc<DirectorySnapshot>) {
    cache()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .remember(identity, data);
}

pub(super) fn remember_if(
    identity: &str,
    data: Arc<DirectorySnapshot>,
    should_remember: impl FnOnce() -> bool,
) -> bool {
    let mut cache = cache().lock().unwrap_or_else(|error| error.into_inner());
    if !should_remember() {
        return false;
    }
    cache.remember(identity, data);
    true
}

pub(super) fn cached(identity: &str) -> Option<DirectorySnapshot> {
    let snapshot = cache()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get(identity);
    snapshot.map(|snapshot| snapshot.as_ref().clone())
}

pub(super) fn update_cached(identity: &str, update: impl FnOnce(&mut DirectorySnapshot)) {
    cache()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .mutate(identity, update);
}

pub(super) fn remove_cached(identity: &str) {
    cache()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .remove(identity);
}

pub(super) fn remove_cached_many(identities: &HashSet<String>) {
    cache()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .remove_many(identities);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::files::{Entry, Listing};

    #[test]
    fn memory_entry_budget_tracks_in_place_mutations() {
        let mut cache = DirectoryCache::default();
        cache.remember(
            "test-budget",
            Arc::new(DirectorySnapshot {
                listing: Listing {
                    path: "applications://folder/test-budget".into(),
                    entries: vec![
                        Entry {
                            path: "applications://app/test-budget-one".into(),
                            name: "one".into(),
                            ..Default::default()
                        },
                        Entry {
                            path: "applications://app/test-budget-two".into(),
                            name: "two".into(),
                            ..Default::default()
                        },
                    ],
                    allowed_ops: vec!["list".into(), "stat".into(), "mkdir".into()],
                    virtual_volume: true,
                    warnings: vec![],
                    default_view: None,
                },
            }),
        );
        assert_eq!(cache.entry_count, 2);

        cache.mutate("test-budget", |snapshot| {
            snapshot.listing.entries.pop();
        });
        assert_eq!(cache.entry_count, 1);

        cache.remove("test-budget");
        assert_eq!(cache.entry_count, 0);
    }
}
