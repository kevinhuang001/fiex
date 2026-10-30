//! Storage implementations for Core-owned virtual views. Presentation and
//! dedicated actions stay in `virtual_resolvers`; adapters never mix them.
//!
//! Every `list` of a path inside the Recent, Applications or Trash views
//! enumerates the real source and stores the complete result under that path.
//! The cache is path-keyed, so subdirectories (Applications user folders and
//! Trash folders) are cached too. `virtual_cache` only reads the last listing;
//! searches walk the cached subtree and never enumerate uncached directories.
use super::{applications, recycle, trash, virtual_resolvers};
#[cfg(not(target_os = "windows"))]
use crate::core::files;
use crate::core::files::Entry;
use crate::core::recent;
use std::path::Path;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock},
};

/// The sidebar's logical Trash place. The platform root is `recyclebin://` on
/// Windows and a real directory on Unix.
const TRASH_ALIAS: &str = "trash://";
/// Safety caps for the startup recursive warm-up.
#[cfg_attr(not(feature = "desktop"), allow(dead_code))]
const MAX_WARM_DIRS: usize = 10_000;
#[cfg_attr(not(feature = "desktop"), allow(dead_code))]
const MAX_WARM_ENTRIES: usize = 500_000;

pub fn entry(path: &str) -> Result<Entry, String> {
    if path == virtual_resolvers::RECENT_URI || path == applications::URI || path == recycle::URI {
        let view = virtual_resolvers::view_for(path).ok_or("unknown builtin VFS")?;
        return Ok(Entry {
            path: path.into(),
            name: view.id().into(),
            is_dir: true,
            readonly: false,
            allowed_ops: view
                .operations(path)
                .into_iter()
                .map(str::to_owned)
                .collect(),
            ..Default::default()
        });
    }
    if path.starts_with(virtual_resolvers::RECENT_URI) {
        return recent::entry_for_alias(path);
    }
    if applications::is_user(path) {
        return applications::stored_entry(path);
    }
    let view = virtual_resolvers::view_for(path).ok_or("unknown builtin VFS")?;
    match view.id() {
        "core:applications" => {
            if applications::is_system_app(path) {
                applications::system_entry(path)
            } else {
                let entries = root_entries(applications::URI)?;
                entries
                    .into_iter()
                    .find(|entry| entry.path == path)
                    .ok_or_else(|| crate::i18n::t("应用不存在").into())
            }
        }
        "core:trash" => trash_entry(path),
        _ => Err("Unsupported: builtin view is not a storage node".into()),
    }
}

pub fn list(path: &str) -> Result<Vec<Entry>, String> {
    let key = virtual_path_key(path).ok_or("Unsupported: not a virtual view path")?;
    // Coalesce concurrent list calls for the same virtual path: the warm-up
    // thread and the first user navigation must not enumerate twice (or block
    // each other on the cache lock). A caller that starts after the leader
    // finishes starts its own refresh as usual.
    if let Some(existing) = { inflight_lock().get(&key).cloned() } {
        return existing.wait();
    }
    let flight = Arc::new(InFlight::new());
    {
        let mut in_flight = inflight_lock();
        if let Some(existing) = in_flight.get(&key).cloned() {
            drop(in_flight);
            return existing.wait();
        }
        in_flight.insert(key.clone(), flight.clone());
    }
    let result = enumerate_and_store(path, &key);
    flight.publish(&result);
    inflight_lock().remove(&key);
    result
}

fn enumerate_and_store(path: &str, key: &str) -> Result<Vec<Entry>, String> {
    let entries = if path == virtual_resolvers::RECENT_URI {
        recent::entries(&recent::Recent::default())
    } else if path == applications::URI {
        applications::list()?
    } else if applications::is_user(path) {
        applications::list_at(path)?
    } else if is_trash_root(path) || path == TRASH_ALIAS || path == recycle::URI {
        trash_list_live()?
    } else {
        trash_subdir_live(path)?
    };
    store(key, &entries);
    Ok(entries)
}

struct InFlight {
    result: Mutex<Option<Result<Vec<Entry>, String>>>,
    ready: Condvar,
}

impl InFlight {
    fn new() -> Self {
        Self {
            result: Mutex::new(None),
            ready: Condvar::new(),
        }
    }

    fn wait(&self) -> Result<Vec<Entry>, String> {
        let mut result = self
            .result
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        while result.is_none() {
            result = self
                .ready
                .wait(result)
                .unwrap_or_else(|error| error.into_inner());
        }
        result
            .clone()
            .unwrap_or_else(|| Err("virtual listing aborted".into()))
    }

    fn publish(&self, result: &Result<Vec<Entry>, String>) {
        let mut slot = self
            .result
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        *slot = Some(result.clone());
        self.ready.notify_all();
    }
}

fn inflight() -> &'static Mutex<HashMap<String, Arc<InFlight>>> {
    static IN_FLIGHT: OnceLock<Mutex<HashMap<String, Arc<InFlight>>>> = OnceLock::new();
    IN_FLIGHT.get_or_init(|| Mutex::new(HashMap::new()))
}

fn inflight_lock() -> MutexGuard<'static, HashMap<String, Arc<InFlight>>> {
    inflight().lock().unwrap_or_else(|error| error.into_inner())
}

/// The Trash is a URI-backed view on Windows and a real directory on Unix.
pub(crate) fn is_trash_root(path: &str) -> bool {
    if path == TRASH_ALIAS || path == recycle::URI {
        return true;
    }
    trash_root_key().is_some_and(|root| root == path)
}

/// True for every listable path owned by the three virtual views (roots and
/// subdirectories). Used by search routing and the builtin adapter.
pub(crate) fn is_virtual_tree_path(path: &str) -> bool {
    virtual_path_key(path).is_some()
}

/// Normalized cache key for a path inside one of the three virtual views.
pub(crate) fn virtual_path_key(path: &str) -> Option<String> {
    if path == virtual_resolvers::RECENT_URI {
        return Some(path.to_string());
    }
    if path == applications::URI || applications::is_user(path) {
        return Some(path.to_string());
    }
    if path == TRASH_ALIAS || path == recycle::URI {
        return Some(trash_listing_root());
    }
    trash_cache_key(path)
}

fn cache() -> &'static Mutex<HashMap<String, Vec<Entry>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Vec<Entry>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cache_lock() -> MutexGuard<'static, HashMap<String, Vec<Entry>>> {
    cache().lock().unwrap_or_else(|error| error.into_inner())
}

fn store(key: &str, entries: &[Entry]) {
    cache_lock().insert(key.to_string(), entries.to_vec());
}

/// Keep a path snapshot coherent after a direct applications move without
/// re-enumerating the platform app list. No-op when the path was never listed.
pub(crate) fn remove_cached_entry(path: &str, entry_path: &str) {
    let Some(key) = virtual_path_key(path) else {
        return;
    };
    if let Some(entries) = cache_lock().get_mut(&key) {
        entries.retain(|entry| entry.path != entry_path);
    }
}

/// Add an entry to an already-cached path snapshot; uncached folders will
/// include it from the mapping on their next live listing.
pub(crate) fn add_cached_entry(path: &str, entry: Entry) {
    let Some(key) = virtual_path_key(path) else {
        return;
    };
    if let Some(entries) = cache_lock().get_mut(&key) {
        if !entries.iter().any(|candidate| candidate.path == entry.path) {
            if path == applications::URI {
                entries.insert(0, entry);
            } else {
                entries.push(entry);
            }
        }
    }
}

/// Drop a folder and all nested snapshots after the folder itself disappears.
pub(crate) fn drop_cached_subtree(path: &str) {
    let Some(key) = virtual_path_key(path) else {
        return;
    };
    let mut cache = cache_lock();
    cache.retain(|candidate, _| !is_subtree_path(&key, candidate));
}

/// Rename a cached root entry in place (folder rename does not re-enumerate).
pub(crate) fn rename_cached_entry(path: &str, old_path: &str, new_entry: Entry) {
    let Some(key) = virtual_path_key(path) else {
        return;
    };
    if let Some(entries) = cache_lock().get_mut(&key) {
        if let Some(entry) = entries.iter_mut().find(|entry| entry.path == old_path) {
            *entry = new_entry;
        }
    }
}

/// Read the last listing of a virtual view path without enumerating. Returns an
/// empty list when the path is valid but has not been listed yet.
pub(crate) fn virtual_cache(path: &str) -> Result<Vec<Entry>, String> {
    let key = virtual_path_key(path).ok_or("Unsupported: not a virtual view path")?;
    Ok(cache_lock().get(&key).cloned().unwrap_or_default())
}

/// Recursively warm every directory under the three roots once at startup.
/// Bounded by `MAX_WARM_DIRS`/`MAX_WARM_ENTRIES`; partial caches are kept when
/// a cap is reached.
#[cfg_attr(not(feature = "desktop"), allow(dead_code))]
pub(crate) fn warm_caches() {
    let mut queue: VecDeque<String> = VecDeque::new();
    queue.push_back(virtual_resolvers::RECENT_URI.to_string());
    queue.push_back(applications::URI.to_string());
    queue.push_back(trash_listing_root());
    let mut visited: HashSet<String> = HashSet::new();
    let mut cached_entries = 0usize;
    while let Some(path) = queue.pop_front() {
        if visited.len() >= MAX_WARM_DIRS || cached_entries >= MAX_WARM_ENTRIES {
            break;
        }
        let Some(key) = virtual_path_key(&path) else {
            continue;
        };
        if !visited.insert(key) {
            continue;
        }
        let Ok(entries) = list(&path) else {
            continue;
        };
        cached_entries = cached_entries.saturating_add(entries.len());
        if cached_entries >= MAX_WARM_ENTRIES {
            break;
        }
        for entry in entries {
            if !entry.is_dir || entry.is_symlink {
                continue;
            }
            if virtual_path_key(&entry.path).is_some() && !visited.contains(&entry.path) {
                queue.push_back(entry.path);
            }
        }
    }
}

/// Root listing used by stat/icon lookups: prefer the cached snapshot, but
/// populate it once if this root was never listed.
fn root_entries(root: &str) -> Result<Vec<Entry>, String> {
    let cached = virtual_cache(root)?;
    if !cached.is_empty() {
        return Ok(cached);
    }
    list(root)
}

/// One name-filter pass over a virtual view's entries.
pub(crate) struct CachedSearch {
    pub(crate) entries: Vec<Entry>,
    pub(crate) truncated: bool,
    pub(crate) scanned: usize,
    pub(crate) skipped: usize,
}

/// Name-filter the cached subtree rooted at `path`. Only cached directories are
/// visited; uncached directories are never enumerated. File contents are never
/// read. `hidden`/`ignores` mirror the normal listing filters.
pub(crate) fn search_view(
    path: &str,
    query: &str,
    hidden: bool,
    ignores: &[String],
    limit: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<CachedSearch, String> {
    let root = virtual_path_key(path).ok_or("Unsupported: not a virtual view path")?;
    let mut result = CachedSearch {
        entries: Vec::new(),
        truncated: false,
        scanned: 0,
        skipped: 0,
    };
    let lowered = query.to_lowercase();
    if lowered.is_empty() {
        return Ok(result);
    }
    let filter = crate::core::ignores::IgnoreSet::new(ignores);
    let mut keys: Vec<String> = {
        let cache = cache_lock();
        cache
            .keys()
            .filter(|key| is_subtree_path(&root, key))
            .cloned()
            .collect()
    };
    // BFS-like: shallower cached directories first, deterministic inside a level.
    keys.sort_by(|left, right| {
        path_depth(left)
            .cmp(&path_depth(right))
            .then_with(|| left.cmp(right))
    });
    for key in keys {
        let entries = cache_lock().get(&key).cloned().unwrap_or_default();
        if push_matches(
            &mut result,
            entries,
            &lowered,
            hidden,
            &filter,
            limit,
            cancelled,
        ) {
            break;
        }
    }
    Ok(result)
}

/// Name-filter the cached subtree without hidden/ignore filtering (tests).
#[cfg(test)]
pub(crate) fn search_cached(
    path: &str,
    query: &str,
    limit: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<CachedSearch, String> {
    search_view(path, query, true, &[], limit, cancelled)
}

/// Shared case-insensitive name filter helper for adapter tests. Search itself
/// goes through `search_view`.
#[cfg(test)]
pub(crate) fn filter_entries(
    entries: Vec<Entry>,
    query: &str,
    limit: usize,
    cancelled: &dyn Fn() -> bool,
) -> CachedSearch {
    let lowered = query.to_lowercase();
    let mut result = CachedSearch {
        entries: Vec::new(),
        truncated: false,
        scanned: 0,
        skipped: 0,
    };
    if !lowered.is_empty() {
        push_matches(
            &mut result,
            entries,
            &lowered,
            true,
            &crate::core::ignores::IgnoreSet::new(&[]),
            limit,
            cancelled,
        );
    }
    result
}

fn push_matches(
    result: &mut CachedSearch,
    entries: Vec<Entry>,
    lowered: &str,
    hidden: bool,
    filter: &crate::core::ignores::IgnoreSet,
    limit: usize,
    cancelled: &dyn Fn() -> bool,
) -> bool {
    for entry in entries {
        if cancelled() {
            return true;
        }
        if !hidden && entry.name.starts_with('.') {
            continue;
        }
        if filter.is_ignored(Path::new(&entry.name)) {
            continue;
        }
        result.scanned += 1;
        if !entry.name.to_lowercase().contains(lowered) {
            continue;
        }
        if result.entries.len() >= limit {
            result.truncated = true;
            return true;
        }
        result.entries.push(entry);
    }
    false
}

fn is_subtree_path(root: &str, path: &str) -> bool {
    if root == path {
        return true;
    }
    let Some(rest) = path.strip_prefix(root) else {
        return false;
    };
    rest.starts_with('/') || rest.starts_with('\\') || root.ends_with("://")
}

fn path_depth(path: &str) -> usize {
    path.matches(['/', '\\']).count()
}

#[cfg(target_os = "windows")]
fn trash_root_key() -> Option<String> {
    Some(recycle::URI.to_string())
}

#[cfg(not(target_os = "windows"))]
fn trash_root_key() -> Option<String> {
    static ROOT: OnceLock<String> = OnceLock::new();
    if let Some(root) = ROOT.get() {
        return Some(root.clone());
    }
    let root = trash::trash_path()?;
    let root = dunce::canonicalize(&root).unwrap_or(root);
    let root = root
        .to_string_lossy()
        .trim_end_matches(['/', '\\'])
        .to_string();
    Some(ROOT.get_or_init(|| root).clone())
}

fn trash_listing_root() -> String {
    trash_root_key().unwrap_or_else(|| recycle::URI.to_string())
}

fn trash_cache_key(path: &str) -> Option<String> {
    let root = trash_root_key()?;
    if path == root {
        return Some(root);
    }
    // Entry paths produced by listing are already under the canonical root;
    // avoid a canonicalize syscall per entry during recursive warm-up.
    if is_subtree_path(&root, path) {
        return Some(path.to_string());
    }
    let candidate = dunce::canonicalize(path)
        .ok()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string());
    is_subtree_path(&root, &candidate).then_some(candidate)
}

fn trash_list_live() -> Result<Vec<Entry>, String> {
    #[cfg(target_os = "windows")]
    {
        recycle::list()
    }
    #[cfg(not(target_os = "windows"))]
    {
        let root = trash::trash_path().ok_or(crate::i18n::t("无法定位废纸篓"))?;
        files::list(&root.to_string_lossy(), true, &[]).map(|listing| listing.entries)
    }
}

fn trash_subdir_live(path: &str) -> Result<Vec<Entry>, String> {
    #[cfg(target_os = "windows")]
    {
        let _ = path;
        Err("Unsupported: Recycle Bin has no listable subdirectories".into())
    }
    #[cfg(not(target_os = "windows"))]
    {
        files::list(path, true, &[]).map(|listing| listing.entries)
    }
}

fn trash_entry(path: &str) -> Result<Entry, String> {
    #[cfg(target_os = "windows")]
    {
        let entries = root_entries(recycle::URI)?;
        entries
            .into_iter()
            .find(|entry| entry.path == path)
            .ok_or_else(|| crate::i18n::t("回收站项目无效").into())
    }
    #[cfg(not(target_os = "windows"))]
    {
        files::entry(Path::new(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str) -> Entry {
        Entry {
            path: format!("/{name}"),
            name: name.into(),
            ..Default::default()
        }
    }

    #[test]
    fn subsections_are_cached_by_path() {
        let path = "applications://user/__cache_test_a__";
        store(path, &[item("one")]);
        store("applications://user/__cache_test_a__/child", &[item("two")]);
        assert_eq!(virtual_cache(path).unwrap().len(), 1);
        assert_eq!(
            virtual_cache("applications://user/__cache_test_a__/child")
                .unwrap()
                .len(),
            1
        );
        assert!(virtual_cache("applications://user/__cache_test_missing__")
            .unwrap()
            .is_empty());
        assert!(virtual_cache("/tmp").is_err());
    }

    #[test]
    fn every_list_overwrites_the_path_snapshot() {
        let path = "applications://user/__cache_test_b__";
        store(path, &[item("one")]);
        assert_eq!(virtual_cache(path).unwrap().len(), 1);
        store(path, &[item("one"), item("two")]);
        assert_eq!(virtual_cache(path).unwrap().len(), 2);
    }

    #[test]
    fn list_and_virtual_cache_share_the_same_entries() {
        let listed = list(virtual_resolvers::RECENT_URI).unwrap();
        let cached = virtual_cache(virtual_resolvers::RECENT_URI).unwrap();
        assert_eq!(cached.len(), listed.len());
        if let Some(first) = listed.first() {
            assert_eq!(cached[0].path, first.path);
        }
    }

    #[test]
    fn search_cached_walks_the_cached_subtree_only() {
        let root = "applications://user/__cache_test_tree__";
        store(root, &[item("Needle One")]);
        store(
            "applications://user/__cache_test_tree__/child",
            &[item("needle-two")],
        );
        store(
            "applications://user/__cache_test_tree_sibling__",
            &[item("needle-sibling")],
        );
        let found = search_cached(root, "needle", 50, &|| false).unwrap();
        let names: Vec<_> = found
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();
        assert_eq!(names, ["Needle One", "needle-two"]);
        assert_eq!(found.scanned, 2);
        assert!(!found.truncated);
        assert_eq!(found.skipped, 0);
    }

    #[test]
    fn search_cached_uses_the_last_listing() {
        let listed = list(virtual_resolvers::RECENT_URI).unwrap();
        let found = search_cached(
            virtual_resolvers::RECENT_URI,
            "__fiex_virtual_cache_probe__",
            50,
            &|| false,
        )
        .unwrap();
        assert!(found.entries.is_empty());
        assert_eq!(found.scanned, listed.len());
        assert!(!found.truncated);
        assert_eq!(found.skipped, 0);
    }

    #[test]
    fn in_flight_list_is_shared() {
        let flight = Arc::new(InFlight::new());
        let waiter = flight.clone();
        let handle = std::thread::spawn(move || waiter.wait());
        std::thread::sleep(std::time::Duration::from_millis(10));
        flight.publish(&Ok(vec![item("one")]));
        assert_eq!(handle.join().unwrap().unwrap().len(), 1);
    }

    #[test]
    fn callers_receive_independent_copies() {
        let path = "applications://user/__cache_test_copy__";
        store(path, &[item("one"), item("two")]);
        let mut first = virtual_cache(path).unwrap();
        first.clear();
        assert_eq!(virtual_cache(path).unwrap().len(), 2);
    }

    #[test]
    fn filter_entries_is_case_insensitive_and_observes_the_limit() {
        let entries = vec![item("Needle One"), item("needle-two"), item("other")];
        let found = filter_entries(entries, "NEEDLE", 10, &|| false);
        assert_eq!(found.entries.len(), 2);
        assert!(!found.truncated);
        assert_eq!(found.scanned, 3);
        assert_eq!(found.skipped, 0);

        let found = filter_entries(
            vec![item("needle-1"), item("needle-2"), item("needle-3")],
            "needle",
            2,
            &|| false,
        );
        assert_eq!(found.entries.len(), 2);
        assert!(found.truncated);
    }

    #[test]
    fn filter_entries_stops_for_cancellation_and_empty_queries() {
        let found = filter_entries(
            vec![item("needle-1"), item("needle-2")],
            "needle",
            10,
            &|| true,
        );
        assert!(found.entries.is_empty());
        assert_eq!(found.scanned, 0);

        let found = filter_entries(vec![item("needle")], "", 10, &|| false);
        assert!(found.entries.is_empty());
    }

    #[test]
    fn virtual_paths_include_roots_and_subdirectories() {
        assert_eq!(
            virtual_path_key(virtual_resolvers::RECENT_URI),
            Some("recent://".into())
        );
        assert!(virtual_path_key(applications::URI).is_some());
        assert!(virtual_path_key("applications://user/abc").is_some());
        assert!(virtual_path_key(recycle::URI).is_some());
        assert!(virtual_path_key(TRASH_ALIAS).is_some());
        assert!(virtual_path_key("/tmp").is_none());
    }
}
