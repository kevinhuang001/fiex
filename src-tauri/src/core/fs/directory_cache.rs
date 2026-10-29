//! Navigation snapshots shared by every storage adapter. No provider sees this
//! cache. Internal list/search/transfer calls never enroll new directories.
use crate::infrastructure::cache::disk;
use crate::core::{
    files::{Entry, Listing},
    ignores::IgnoreSet,
};
use std::{
    collections::{BTreeMap, HashSet, VecDeque},
    path::Path,
    sync::{Mutex, OnceLock},
};

const MAX_DIRECTORIES: usize = 10_000;
const MAX_ENTRIES: usize = 500_000;
fn cache() -> &'static Mutex<BTreeMap<String, Listing>> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, Listing>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}
fn key(path: &str) -> String {
    // A URI's mount ID alone is not sufficient when a saved connection changes
    // accounts, settings or provider. Include its current authorized identity.
    if crate::plugins::mounts::handles(path) {
        use sha2::{Digest, Sha256};
        if let Ok((_, source)) = crate::plugins::mounts::resolve(path) {
            let identity = match source {
                crate::plugins::mounts::Source::Connection(connection) =>
                    serde_json::to_vec(&connection).unwrap_or_default(),
                crate::plugins::mounts::Source::View(view) =>
                    serde_json::to_vec(&view.descriptor).unwrap_or_default(),
            };
            return format!("mount:{}:{}", hex::encode(Sha256::digest(&identity)), path);
        }
    }
    if let Some(route) = crate::usbfs::route(path) {
        return format!(
            "usb:{}\0{}",
            crate::usbfs::generation(&route).unwrap_or_else(|_| "offline".into()),
            path
        );
    }
    crate::core::volumes::virtual_storage::virtual_path_key(path).unwrap_or_else(|| path.into())
}
fn validate_mount(path: &str) -> Result<(), String> {
    // Metadata reads must not stat a remote provider: that would delay first paint.
    if crate::plugins::mounts::handles(path) {
        crate::plugins::mounts::resolve(path)?;
    }
    Ok(())
}
pub(crate) fn store(path: &str, listing: &Listing) {
    let key = key(path);
    {
        let mut values = cache().lock().unwrap_or_else(|e| e.into_inner());
        let mut total: usize = values.values().map(|v| v.entries.len()).sum();
        total = total.saturating_sub(values.get(&key).map_or(0, |v| v.entries.len()));
        // Evict older snapshots rather than silently declining to cache new
        // directories after an extended navigation session.
        while (values.len() >= MAX_DIRECTORIES || total.saturating_add(listing.entries.len()) > MAX_ENTRIES)
            && !values.is_empty()
        {
            let Some(victim) = values.keys().find(|item| *item != &key).cloned() else { break };
            if let Some(value) = values.remove(&victim) {
                total = total.saturating_sub(value.entries.len());
            }
        }
        if listing.entries.len() <= MAX_ENTRIES {
            values.insert(key.clone(), listing.clone());
        }
    }
    disk::store(&key, listing);
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
    let key = key(path);
    let listing = cache().lock().unwrap_or_else(|e| e.into_inner())
        .get(&key).cloned()
        .or_else(|| disk::load::<Listing>(&key));
    Ok(listing.map(|listing| filtered(listing, hidden, ignores)))
}
/// Refresh only an already visited directory. Search never enrolls a cold one.
pub(crate) fn refresh(path: &str) -> Result<(), String> {
    if read(path, true, &[])?.is_none() {
        return Ok(());
    }
    let broker = super::trusted(path, "core:cache-refresh")?;
    broker.require(super::FsOperation::List, false)?;
    let listing = broker.adapter()?.desktop_listing(path, true, &[])?;
    store(path, &listing);
    Ok(())
}
/// Revalidate only enrolled directories reachable from the current root.
pub(crate) fn refresh_tree(root: &str, cancelled: &dyn Fn() -> bool) -> Result<(), String> {
    let mut queue = VecDeque::from([root.to_string()]);
    let mut visited = HashSet::new();
    while let Some(path) = queue.pop_front() {
        if cancelled() {
            return Ok(());
        }
        if !visited.insert(path.clone()) {
            continue;
        }
        refresh(&path)?;
        let listing = read(&path, true, &[])?;
        if let Some(listing) = listing {
            for entry in listing.entries {
                if entry.is_dir && !entry.is_symlink
                    && read(&entry.path, true, &[])?.is_some() {
                    queue.push_back(entry.path);
                }
            }
        }
    }
    Ok(())
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
    validate_mount(path)?;
    let snapshots = cache().lock().unwrap_or_else(|e| e.into_inner()).clone();
    let matcher = grep_regex::RegexMatcherBuilder::new()
        .case_insensitive(true)
        .fixed_strings(true)
        .build(query)
        .map_err(|e| e.to_string())?;
    use grep_matcher::Matcher;
    let filter = IgnoreSet::new(ignores);
    let mut queue = VecDeque::from([key(path)]);
    let mut visited = HashSet::new();
    while let Some(path) = queue.pop_front() {
        if cancelled() {
            break;
        }
        if !visited.insert(path.clone()) {
            continue;
        }
        let Some(listing) = snapshots.get(&path).cloned()
            .or_else(|| disk::load::<Listing>(&path)) else {
            continue;
        };
        for entry in &listing.entries {
            if cancelled() {
                return Ok(());
            }
            if !visible(entry, hidden, &filter) {
                continue;
            }
            if entry.is_dir && !entry.is_symlink {
                queue.push_back(key(&entry.path));
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
    disk::remove(&key(path));
    if let Some(listing) = cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_mut(&key(path))
    {
        listing.entries.retain(|entry| entry.path != entry_path);
    }
}
pub(crate) fn add_entry(path: &str, entry: Entry) {
    disk::remove(&key(path));
    if let Some(listing) = cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_mut(&key(path))
    {
        if !listing.entries.iter().any(|item| item.path == entry.path) {
            listing.entries.push(entry);
        }
    }
}
pub(crate) fn drop_subtree(path: &str) {
    let root = key(path);
    // Remove the previously visited disk graph, without enumerating cold FS/VFS
    // directories or relying on URI string prefix matching.
    let mut pending = VecDeque::from([path.to_string()]);
    let mut removed = HashSet::new();
    while let Some(current) = pending.pop_front() {
        let identity = key(&current);
        if !removed.insert(identity.clone()) { continue; }
        if let Some(listing) = disk::load::<Listing>(&identity) {
            for entry in listing.entries {
                if entry.is_dir && !entry.is_symlink { pending.push_back(entry.path); }
            }
        }
        disk::remove(&identity);
    }
    cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|path, _| {
            !(path == &root
                || path
                    .strip_prefix(&root)
                    .is_some_and(|tail| root.ends_with("://") || tail.starts_with(['/', '\\'])))
        });
}
pub(crate) fn rename_entry(path: &str, old_path: &str, entry: Entry) {
    disk::remove(&key(path));
    if let Some(listing) = cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_mut(&key(path))
    {
        if let Some(item) = listing
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
