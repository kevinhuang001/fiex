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
    collections::{HashSet, VecDeque},
    path::Path,
    sync::Arc,
};

mod memory;
use memory::{cached, remove_cached, remove_cached_many, snapshot, update_cached};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DirectorySnapshot {
    pub listing: Listing,
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
    // Providers may attach owner-specific validation to cached snapshots.
    // Errors preserve the snapshot; live list refresh remains responsible for
    // reporting provider failures.
    let _ = crate::core::volumes::native_vfs::revalidate_cached_listing(
        path,
        &mut data.listing.entries,
    );
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
    if !memory::remember_if(&identity, Arc::clone(&data), || {
        super::changes::revision(path) == revision
    }) {
        return;
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
