//! Complete VFS structure snapshots. Local/USB storage always bypasses this service.
use super::cache_identity::validate_mount;
pub(crate) use super::cache_identity::{enabled, key};
use crate::core::{
    files::{Entry, Listing},
    ignores::IgnoreSet,
};
use crate::infrastructure::cache::disk;
use crate::infrastructure::cache::lru::LruMap;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashSet, VecDeque},
    path::Path,
    sync::{Arc, Mutex, OnceLock},
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DirectorySnapshot {
    pub listing: Listing,
}

fn snapshot(key: &str) -> Option<DirectorySnapshot> {
    let memory = cache().lock().unwrap_or_else(|e| e.into_inner()).get(key);
    if let Some(data) = memory {
        return Some(data.as_ref().clone());
    }
    let data = disk::load::<DirectorySnapshot>(key)?;
    remember(key, Arc::new(data.clone()));
    Some(data)
}
pub(crate) fn read_snapshot(
    path: &str,
    hidden: bool,
    ignores: &[String],
) -> Result<Option<DirectorySnapshot>, String> {
    if !enabled(path) {
        return Ok(None);
    }
    validate_mount(path)?;
    let mut data = match snapshot(&key(path)) {
        Some(data) => data,
        None => return Ok(None),
    };
    super::mutation_overlay::paint(path, &mut data.listing);
    data.listing = filtered(data.listing, hidden, ignores);
    Ok(Some(data))
}

/// Find an item in a previously rendered directory snapshot. This is a UI
/// hint only: callers still reauthorize every operation before execution.
#[cfg(test)]
pub(crate) fn cached_entry(directory: &str, path: &str) -> Result<Option<Entry>, String> {
    Ok(read_snapshot(directory, true, &[])?.and_then(|snapshot| {
        snapshot
            .listing
            .entries
            .into_iter()
            .find(|entry| entry.path == path)
    }))
}

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
fn remember(identity: &str, data: Arc<DirectorySnapshot>) {
    cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remember(identity, data);
}
fn cached(identity: &str) -> Option<DirectorySnapshot> {
    let snapshot = cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(identity);
    snapshot.map(|snapshot| snapshot.as_ref().clone())
}
fn update_cached(identity: &str, update: impl FnOnce(&mut DirectorySnapshot)) {
    cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .mutate(identity, update);
}
fn remove_cached(identity: &str) {
    cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(identity);
}
fn remove_cached_many(identities: &HashSet<String>) {
    cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove_many(identities);
}
#[cfg(test)]
pub(crate) fn store(path: &str, listing: &Listing) {
    let revision = super::changes::revision(path);
    store_at_revision(path, listing, revision);
}

pub(crate) fn store_at_revision(path: &str, listing: &Listing, revision: u64) {
    if !enabled(path) {
        return;
    }
    let identity = key(path);
    if super::changes::revision(path) != revision {
        return;
    }
    let mut merged = listing.clone();
    if !super::mutation_overlay::reconcile_at_revision(path, &mut merged, revision) {
        return;
    }
    let data = Arc::new(DirectorySnapshot { listing: merged });
    {
        let mut values = cache().lock().unwrap_or_else(|e| e.into_inner());
        if super::changes::revision(path) != revision {
            return;
        }
        values.remember(&identity, Arc::clone(&data));
    }
    let persistence_path = path.to_owned();
    let persistence_identity = identity.clone();
    disk::store_shared_async(identity, data, move || {
        super::changes::revision(&persistence_path) == revision
            && key(&persistence_path) == persistence_identity
    });
}
pub(crate) fn filtered(mut listing: Listing, hidden: bool, ignores: &[String]) -> Listing {
    let filter = IgnoreSet::new(ignores);
    listing
        .entries
        .retain(|entry| visible(entry, hidden, &filter));
    listing
}
fn visible(entry: &Entry, hidden: bool, filter: &IgnoreSet) -> bool {
    (hidden || !entry.name.starts_with('.')) && !filter.is_ignored(Path::new(&entry.name))
}
#[cfg(test)]
pub(crate) fn read(
    path: &str,
    hidden: bool,
    ignores: &[String],
) -> Result<Option<Listing>, String> {
    // Resolve the current mount/scope before exposing metadata from memory.
    validate_mount(path)?;
    Ok(read_snapshot(path, hidden, ignores)?.map(|data| data.listing))
}
/// Traverse parent/child edges, not URI prefixes: deleted, hidden and ignored
/// folders cannot leak stale descendants into results. Emit hits as we scan.
pub(crate) fn search(
    path: &str,
    query: &str,
    hidden: bool,
    ignores: &[String],
    cancelled: &dyn Fn() -> bool,
    emit: &mut dyn FnMut(Entry),
) -> Result<(), String> {
    if !enabled(path) {
        return Ok(());
    }
    validate_mount(path)?;
    let matcher = grep_regex::RegexMatcherBuilder::new()
        .case_insensitive(true)
        .fixed_strings(true)
        .build(query)
        .map_err(|e| e.to_string())?;
    use grep_matcher::Matcher;
    let filter = IgnoreSet::new(ignores);
    let mut queue = VecDeque::from([path.to_string()]);
    let mut visited = HashSet::new();
    while let Some(path) = queue.pop_front() {
        if cancelled() {
            break;
        }
        if !visited.insert(path.clone()) {
            continue;
        }
        let Some(data) = read_snapshot(&path, true, &[])? else {
            continue;
        };
        for entry in &data.listing.entries {
            if cancelled() {
                return Ok(());
            }
            if !visible(entry, hidden, &filter) {
                continue;
            }
            if entry.is_dir && !entry.is_symlink {
                queue.push_back(entry.path.clone());
            }
            if matcher
                .is_match(entry.name.as_bytes())
                .map_err(|e| e.to_string())?
            {
                emit(entry.clone());
            }
        }
    }
    Ok(())
}
pub(crate) fn remove_entry(path: &str, entry_path: &str) {
    // A removed child may have previously visited descendants; invalidate
    // their entire graph, not only the parent row.
    if !enabled(path) {
        return;
    }
    drop_subtree(entry_path);
    super::mutation_overlay::remove(path, entry_path);
    super::decoration_cache::invalidate(&key(path));
    disk::remove_many_async(vec![key(path)]);
    update_cached(&key(path), |data| {
        data.listing
            .entries
            .retain(|entry| entry.path != entry_path);
    });
}
pub(crate) fn add_entry(path: &str, entry: Entry) {
    if !enabled(path) {
        return;
    }
    super::mutation_overlay::upsert(path, entry.clone());
    super::decoration_cache::invalidate(&key(path));
    disk::remove_many_async(vec![key(path)]);
    update_cached(&key(path), |data| {
        if let Some(item) = data
            .listing
            .entries
            .iter_mut()
            .find(|item| item.path == entry.path)
        {
            *item = entry;
        } else {
            data.listing.entries.push(entry);
        }
    });
}
pub(crate) fn drop_subtree(path: &str) {
    if !enabled(path) {
        return;
    }
    // Walk only the already-visited graph: arbitrary string prefixes would
    // incorrectly match unrelated VFS URIs, and miss a mount's root children.
    let mut pending = VecDeque::from([path.to_string()]);
    let mut removed = HashSet::new();
    while let Some(current) = pending.pop_front() {
        let identity = key(&current);
        if !removed.insert(identity.clone()) {
            continue;
        }
        let memory = cached(&identity);
        if let Some(snapshot) = memory.or_else(|| disk::load::<DirectorySnapshot>(&identity)) {
            for entry in snapshot.listing.entries {
                if entry.is_dir && !entry.is_symlink {
                    pending.push_back(entry.path);
                }
            }
        }
        // A deleted or renamed subtree must not resurrect old child overlays.
        super::mutation_overlay::discard(&current);
        super::decoration_cache::invalidate(&identity);
    }
    disk::remove_many_async(removed.iter().cloned().collect());
    remove_cached_many(&removed);
}
pub(crate) fn rename_entry(path: &str, old_path: &str, entry: Entry) {
    // A folder rename changes every descendant URI/path in the old subtree.
    if !enabled(path) {
        return;
    }
    drop_subtree(old_path);
    super::mutation_overlay::remove(path, old_path);
    super::mutation_overlay::upsert(path, entry.clone());
    super::decoration_cache::invalidate(&key(path));
    disk::remove_many_async(vec![key(path)]);
    update_cached(&key(path), |data| {
        data.listing.entries.retain(|item| item.path != old_path);
        if let Some(item) = data
            .listing
            .entries
            .iter_mut()
            .find(|item| item.path == entry.path)
        {
            *item = entry;
        } else {
            data.listing.entries.push(entry);
        }
    });
}

#[cfg(test)]
mod tests;

pub(crate) fn invalidate_directory(path: &str) {
    if !enabled(path) {
        return;
    }
    let identity = key(path);
    super::mutation_overlay::discard(path);
    remove_cached(&identity);
    disk::remove_many_async(vec![identity.clone()]);
    super::decoration_cache::invalidate(&identity);
}
