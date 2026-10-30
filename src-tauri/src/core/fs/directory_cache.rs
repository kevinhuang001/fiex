//! Complete VFS structure snapshots. Local/USB storage always bypasses this service.
use super::cache_identity::validate_mount;
pub(crate) use super::cache_identity::{enabled, key};
use crate::core::{
    files::{Entry, Listing},
    ignores::IgnoreSet,
};
use crate::infrastructure::cache::disk;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet, VecDeque},
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, OnceLock,
    },
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DirectorySnapshot {
    pub listing: Listing,
    #[serde(skip)]
    last_access: u64,
}

static ACCESS_CLOCK: AtomicU64 = AtomicU64::new(1);
fn next_access() -> u64 {
    ACCESS_CLOCK.fetch_add(1, Ordering::Relaxed)
}
fn snapshot(key: &str) -> Option<DirectorySnapshot> {
    let memory = {
        let mut cache = cache().lock().unwrap_or_else(|e| e.into_inner());
        cache.get_mut(key).map(|entry| {
            entry.last_access = next_access();
            entry.clone()
        })
    };
    let data = memory.or_else(|| disk::load::<DirectorySnapshot>(key))?;
    remember(key, &data);
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
    data.listing = filtered(data.listing, hidden, ignores);
    Ok(Some(data))
}

const MAX_DIRECTORIES: usize = 10_000;
const MAX_ENTRIES: usize = 500_000;
fn cache() -> &'static Mutex<BTreeMap<String, DirectorySnapshot>> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, DirectorySnapshot>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}
fn remember(identity: &str, data: &DirectorySnapshot) {
    let mut values = cache().lock().unwrap_or_else(|e| e.into_inner());
    values.remove(identity);
    let mut total: usize = values.values().map(|v| v.listing.entries.len()).sum();
    while (values.len() >= MAX_DIRECTORIES
        || total.saturating_add(data.listing.entries.len()) > MAX_ENTRIES)
        && !values.is_empty()
    {
        let victim = values
            .iter()
            .min_by_key(|(_, v)| v.last_access)
            .map(|(k, _)| k.clone())
            .unwrap();
        if let Some(value) = values.remove(&victim) {
            total = total.saturating_sub(value.listing.entries.len());
        }
    }
    if data.listing.entries.len() <= MAX_ENTRIES {
        values.insert(identity.into(), data.clone());
    }
}
pub(crate) fn store(path: &str, listing: &Listing) {
    if !enabled(path) {
        return;
    }
    let identity = key(path);
    let data = DirectorySnapshot {
        listing: listing.clone(),
        last_access: next_access(),
    };
    remember(&identity, &data);
    disk::store(&identity, &data);
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
    super::decoration_cache::invalidate(&key(path));
    disk::remove(&key(path));
    if let Some(data) = cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_mut(&key(path))
    {
        data.listing
            .entries
            .retain(|entry| entry.path != entry_path);
    }
}
pub(crate) fn add_entry(path: &str, entry: Entry) {
    if !enabled(path) {
        return;
    }
    super::decoration_cache::invalidate(&key(path));
    disk::remove(&key(path));
    if let Some(data) = cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_mut(&key(path))
    {
        if !data
            .listing
            .entries
            .iter()
            .any(|item| item.path == entry.path)
        {
            data.listing.entries.push(entry);
        }
    }
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
        let memory = cache()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&identity)
            .cloned();
        if let Some(snapshot) = memory.or_else(|| disk::load::<DirectorySnapshot>(&identity)) {
            for entry in snapshot.listing.entries {
                if entry.is_dir && !entry.is_symlink {
                    pending.push_back(entry.path);
                }
            }
        }
        super::decoration_cache::invalidate(&identity);
        disk::remove(&identity);
    }
    cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|identity, _| !removed.contains(identity));
}
pub(crate) fn rename_entry(path: &str, old_path: &str, entry: Entry) {
    // A folder rename changes every descendant URI/path in the old subtree.
    if !enabled(path) {
        return;
    }
    drop_subtree(old_path);
    super::decoration_cache::invalidate(&key(path));
    disk::remove(&key(path));
    if let Some(data) = cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_mut(&key(path))
    {
        if let Some(item) = data
            .listing
            .entries
            .iter_mut()
            .find(|item| item.path == old_path)
        {
            *item = entry;
        }
    }
}

#[cfg(test)]
mod tests;

pub(crate) fn invalidate_directory(path: &str) {
    if !enabled(path) {
        return;
    }
    let identity = key(path);
    cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&identity);
    disk::remove(&identity);
    super::decoration_cache::invalidate(&identity);
}
