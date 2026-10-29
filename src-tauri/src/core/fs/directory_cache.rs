//! Authorized navigation facade. Cache storage and disk format live exclusively
//! in infrastructure/cache; adapters and plugins do not know caching exists.
use crate::core::{
    files::{Entry, Listing},
    ignores::IgnoreSet,
};
use crate::infrastructure::cache::directory as storage;
use crate::plugins::resolvers::decorations::Decoration;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::Path,
};

fn key(path: &str) -> String {
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
    if crate::plugins::mounts::handles(path) {
        crate::plugins::mounts::resolve(path)?;
    }
    Ok(())
}
pub(crate) fn store(path: &str, listing: &Listing) {
    storage::store(&key(path), listing);
}
pub(crate) fn store_decorations(path: &str, entries: &[Entry], values: Vec<Decoration>) {
    storage::set_decorations(&key(path), entries, values);
}
pub(crate) fn presentations(path: &str) -> Result<HashMap<String, Decoration>, String> {
    validate_mount(path)?;
    Ok(storage::get(&key(path))
        .map(|v| v.decorations)
        .unwrap_or_default())
}
pub(crate) fn filtered(mut listing: Listing, hidden: bool, ignores: &[String]) -> Listing {
    let filter = IgnoreSet::new(ignores);
    listing.entries.retain(|entry| {
        (hidden || !entry.name.starts_with('.')) && !filter.is_ignored(Path::new(&entry.name))
    });
    listing
}
pub(crate) fn read(
    path: &str,
    hidden: bool,
    ignores: &[String],
) -> Result<Option<Listing>, String> {
    validate_mount(path)?;
    Ok(storage::get(&key(path)).map(|v| filtered(v.listing, hidden, ignores)))
}
/// Refresh only a directory that has already been visited by navigation.
pub(crate) fn refresh(path: &str) -> Result<(), String> {
    validate_mount(path)?;
    if !storage::contains(&key(path)) {
        return Ok(());
    }
    let broker = super::trusted(path, "core:cache-refresh")?;
    broker.require(super::FsOperation::List, false)?;
    let listing = broker.adapter()?.desktop_listing(path, true, &[])?;
    store(path, &listing);
    Ok(())
}
pub(crate) fn refresh_tree(root: &str, cancelled: &dyn Fn() -> bool) -> Result<(), String> {
    let mut queue = VecDeque::from([root.to_string()]);
    let mut visited = HashSet::new();
    while let Some(path) = queue.pop_front() {
        if cancelled() || !visited.insert(path.clone()) {
            continue;
        }
        refresh(&path)?;
        if let Some(listing) = storage::get(&key(&path)) {
            for entry in listing.listing.entries {
                if entry.is_dir && !entry.is_symlink && storage::contains(&key(&entry.path)) {
                    queue.push_back(entry.path);
                }
            }
        }
    }
    Ok(())
}
/// Cached filename search traverses real parent-child edges, never URI prefixes.
pub(crate) fn search(
    path: &str,
    query: &str,
    hidden: bool,
    ignores: &[String],
    cancelled: &dyn Fn() -> bool,
    emit: &mut dyn FnMut(Entry),
) -> Result<(), String> {
    validate_mount(path)?;
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
        let Some(snapshot) = storage::get(&path) else {
            continue;
        };
        for entry in snapshot.listing.entries {
            if cancelled() {
                return Ok(());
            }
            if !hidden && entry.name.starts_with('.') {
                continue;
            }
            if filter.is_ignored(Path::new(&entry.name)) {
                continue;
            }
            if entry.is_dir && !entry.is_symlink {
                queue.push_back(key(&entry.path));
            }
            if matcher
                .is_match(entry.name.as_bytes())
                .map_err(|e| e.to_string())?
            {
                emit(entry);
            }
        }
    }
    Ok(())
}
pub(crate) fn remove_entry(path: &str, entry_path: &str) {
    storage::mutate(&key(path), |listing| {
        listing.entries.retain(|e| e.path != entry_path)
    });
}
pub(crate) fn add_entry(path: &str, entry: Entry) {
    storage::mutate(&key(path), |listing| {
        if !listing.entries.iter().any(|item| item.path == entry.path) {
            listing.entries.push(entry);
        }
    });
}
pub(crate) fn rename_entry(path: &str, old_path: &str, entry: Entry) {
    storage::mutate(&key(path), |listing| {
        if let Some(item) = listing.entries.iter_mut().find(|e| e.path == old_path) {
            *item = entry;
        }
    });
}
pub(crate) fn drop_subtree(path: &str) {
    let root = key(path);
    storage::remove_if(|value| {
        value == root
            || value
                .strip_prefix(&root)
                .is_some_and(|tail| root.ends_with("://") || tail.starts_with(['/', '\\']))
    });
}
#[cfg(test)]
mod tests;
