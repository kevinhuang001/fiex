//! Navigation snapshots shared by every storage adapter. No provider sees this
//! cache. Internal list/search/transfer calls never enroll new directories.
use crate::core::{
    files::{Entry, Listing},
    ignores::IgnoreSet,
};
use crate::infrastructure::cache::disk;
use crate::plugins::resolvers::decorations::{Decoration, DecorationEntry};
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
    #[serde(default)]
    pub decorations: Vec<Decoration>,
    #[serde(default)]
    presentation_stamp: String,
    #[serde(skip)]
    last_access: u64,
}

fn stamp() -> String {
    use sha2::{Digest, Sha256};
    let catalog = crate::plugins::host::catalog();
    let versions = catalog
        .iter()
        .map(|p| {
            (
                &p.id,
                &p.version,
                p.enabled,
                p.approved,
                &p.granted_permissions,
            )
        })
        .collect::<Vec<_>>();
    let providers = serde_json::to_vec(&versions).unwrap_or_default();
    let prefs = serde_json::to_vec(&crate::plugins::preferences::snapshot()).unwrap_or_default();
    let mut hash = Sha256::new();
    hash.update(crate::i18n::locale().as_bytes());
    hash.update(&providers);
    hash.update(&prefs);
    hex::encode(hash.finalize())
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
    memory
        .or_else(|| disk::load::<DirectorySnapshot>(key))
        .map(|mut data| {
            if !data.decorations.is_empty() && data.presentation_stamp != stamp() {
                data.decorations.clear();
            }
            data
        })
}
pub(crate) fn read_snapshot(
    path: &str,
    hidden: bool,
    ignores: &[String],
) -> Result<Option<DirectorySnapshot>, String> {
    validate_mount(path)?;
    let mut data = match snapshot(&key(path)) {
        Some(data) => data,
        None => return Ok(None),
    };
    data.listing = filtered(data.listing, hidden, ignores);
    let visible: HashSet<_> = data
        .listing
        .entries
        .iter()
        .map(|e| e.path.as_str())
        .collect();
    data.decorations
        .retain(|d| visible.contains(d.path.as_str()));
    Ok(Some(data))
}

/// Decoration responses attach only to already-visited directories, and only
/// when they still describe the same entries the user actually viewed.
pub(crate) fn store_decorations(
    path: &str,
    displayed: &[DecorationEntry],
    decorations: &[Decoration],
) {
    let identity = key(path);
    let Some(mut data) = snapshot(&identity) else {
        return;
    };
    let valid = displayed.iter().all(|incoming| {
        data.listing.entries.iter().any(|entry| {
            entry.path == incoming.path
                && entry.name == incoming.name
                && entry.is_dir == incoming.is_dir
                && entry.extension == incoming.extension
                && entry.mime == incoming.mime
                && entry.readonly == incoming.readonly
        })
    });
    if !valid {
        return;
    }
    let targets: HashSet<_> = displayed.iter().map(|e| e.path.as_str()).collect();
    data.decorations
        .retain(|d| !targets.contains(d.path.as_str()));
    data.decorations.extend(decorations.iter().cloned());
    data.presentation_stamp = stamp();
    data.last_access = next_access();
    {
        let mut cache = cache().lock().unwrap_or_else(|e| e.into_inner());
        cache.insert(identity.clone(), data.clone());
    }
    disk::store(&identity, &data);
}

const MAX_DIRECTORIES: usize = 10_000;
const MAX_ENTRIES: usize = 500_000;
fn cache() -> &'static Mutex<BTreeMap<String, DirectorySnapshot>> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, DirectorySnapshot>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}
fn key(path: &str) -> String {
    // A URI's mount ID alone is not sufficient when a saved connection changes
    // accounts, settings or provider. Include its current authorized identity.
    if crate::plugins::mounts::handles(path) {
        use sha2::{Digest, Sha256};
        if let Ok(uri) = crate::plugins::mounts::parse(path) {
            // Cache identity is built from the public connection record: do
            // not hydrate secrets or trigger OS keyring prompts on cache reads.
            let identity = crate::remote::load_stored_public()
                .into_iter()
                .find(|conn| conn.mount_id == uri.mount_id)
                .and_then(|conn| {
                    serde_json::to_vec(&(
                        conn.id,
                        conn.plugin_id,
                        conn.kind,
                        conn.settings,
                        conn.mount_id,
                        conn.cache_revision,
                    ))
                    .ok()
                })
                .or_else(|| {
                    crate::plugins::mounts::view(&uri.mount_id)
                        .ok()
                        .and_then(|view| serde_json::to_vec(&view.descriptor).ok())
                });
            if let Some(identity) = identity {
                return format!("mount:{}:{}", hex::encode(Sha256::digest(&identity)), path);
            }
        }
    }
    if let Some(route) = crate::usbfs::route(path) {
        return format!(
            "usb:{}\0{}",
            crate::usbfs::generation(&route).unwrap_or_else(|_| "offline".into()),
            path
        );
    }
    if let Some(virtual_path) = crate::core::volumes::virtual_storage::virtual_path_key(path) {
        return virtual_path;
    }
    // ResourceBroker canonicalizes local paths on macOS (/var is a symlink to
    // /private/var). Use the same identity for the original UI path and the
    // broker's normalized path, or a successful listing becomes a cache miss.
    let original = Path::new(path);
    if original.is_absolute() {
        // Resolve the nearest existing ancestor so delete/rename invalidation
        // finds the same cache key after a leaf directory no longer exists.
        for ancestor in original.ancestors() {
            if let Ok(canonical) = dunce::canonicalize(ancestor) {
                if let Ok(suffix) = original.strip_prefix(ancestor) {
                    // Never normalize unresolved .. components into another
                    // directory's cached identity.
                    if suffix
                        .components()
                        .all(|part| matches!(part, std::path::Component::Normal(_)))
                    {
                        return canonical.join(suffix).to_string_lossy().into_owned();
                    }
                }
            }
        }
    }
    path.into()
}
fn validate_mount(path: &str) -> Result<(), String> {
    // Reauthorize every cache read, but never hydrate remote credentials:
    // providers see exactly the same permissions while cached navigation
    // avoids a keyring prompt on every paint and filename search.
    if crate::plugins::mounts::handles(path) {
        let uri = crate::plugins::mounts::parse(path)?;
        if let Some(connection) = crate::remote::load_stored_public()
            .into_iter()
            .find(|conn| conn.mount_id == uri.mount_id)
        {
            crate::remote::ensure_provider_access(&connection)?;
        } else {
            crate::plugins::mounts::view(&uri.mount_id)?;
        }
    }
    Ok(())
}
pub(crate) fn store(path: &str, listing: &Listing) {
    let identity = key(path);
    let previous = snapshot(&identity);
    // Keep the old presentation when revalidation returns identical entries.
    let unchanged = previous.as_ref().is_some_and(|old| {
        let mut left = old.listing.entries.clone();
        let mut right = listing.entries.clone();
        left.sort_by(|a, b| a.path.cmp(&b.path));
        right.sort_by(|a, b| a.path.cmp(&b.path));
        serde_json::to_value(left).ok() == serde_json::to_value(right).ok()
    });
    let data = DirectorySnapshot {
        listing: listing.clone(),
        decorations: if unchanged {
            previous
                .as_ref()
                .map(|v| v.decorations.clone())
                .unwrap_or_default()
        } else {
            Vec::new()
        },
        presentation_stamp: if unchanged {
            previous.map(|v| v.presentation_stamp).unwrap_or_default()
        } else {
            String::new()
        },
        last_access: next_access(),
    };
    {
        let mut values = cache().lock().unwrap_or_else(|e| e.into_inner());
        let mut total: usize = values.values().map(|v| v.listing.entries.len()).sum();
        total = total.saturating_sub(values.get(&identity).map_or(0, |v| v.listing.entries.len()));
        while (values.len() >= MAX_DIRECTORIES
            || total.saturating_add(listing.entries.len()) > MAX_ENTRIES)
            && !values.is_empty()
        {
            let Some(victim) = values
                .iter()
                .filter(|(key, _)| *key != &identity)
                .min_by_key(|(_, record)| record.last_access)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            if let Some(value) = values.remove(&victim) {
                total = total.saturating_sub(value.listing.entries.len());
            }
        }
        if listing.entries.len() <= MAX_ENTRIES {
            values.insert(identity.clone(), data.clone());
        }
    }
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
                if entry.is_dir && !entry.is_symlink && read(&entry.path, true, &[])?.is_some() {
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
        let Some(data) = snapshots
            .get(&path)
            .cloned()
            .or_else(|| disk::load::<DirectorySnapshot>(&path))
        else {
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
    // A removed child may have previously visited descendants; invalidate
    // their entire graph, not only the parent row.
    drop_subtree(entry_path);
    disk::remove(&key(path));
    if let Some(data) = cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_mut(&key(path))
    {
        data.listing
            .entries
            .retain(|entry| entry.path != entry_path);
        data.decorations.clear();
    }
}
pub(crate) fn add_entry(path: &str, entry: Entry) {
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
            data.decorations.clear();
        }
    }
}
pub(crate) fn drop_subtree(path: &str) {
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
        disk::remove(&identity);
    }
    cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|identity, _| !removed.contains(identity));
}
pub(crate) fn rename_entry(path: &str, old_path: &str, entry: Entry) {
    // A folder rename changes every descendant URI/path in the old subtree.
    drop_subtree(old_path);
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
            data.decorations.clear();
        }
    }
}

#[cfg(test)]
mod tests;
