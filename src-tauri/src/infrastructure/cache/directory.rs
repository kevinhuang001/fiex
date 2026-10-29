//! Unified directory/presentation snapshots for visited FS and VFS directories.
//! This module never performs a Provider RPC or checks permissions: the
//! authorized resource service owns those decisions and supplies stable keys.
use crate::{
    core::files::{Entry, Listing},
    plugins::resolvers::decorations::Decoration,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, VecDeque},
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex, OnceLock,
    },
    time::{SystemTime, UNIX_EPOCH},
};
const MAX_MEMORY_DIRECTORIES: usize = 512;
const MAX_DISK_FILES: usize = 4096;
const MAX_DISK_BYTES: u64 = 512 * 1024 * 1024;
const MAX_SNAPSHOT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_MEMORY_ENTRIES: usize = 300_000;

#[derive(Clone, Serialize, Deserialize)]
pub struct DirectorySnapshot {
    pub key: String,
    pub listing: Listing,
    pub decorations: HashMap<String, Decoration>,
    pub captured_at: u64,
}
#[derive(Default)]
struct CacheState {
    values: HashMap<String, DirectorySnapshot>,
    recent: VecDeque<String>,
    entries: usize,
}
fn cache() -> &'static Mutex<CacheState> {
    static C: OnceLock<Mutex<CacheState>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(CacheState::default()))
}
fn root() -> Option<PathBuf> {
    dirs::cache_dir().map(|p| p.join("fiex").join("directory-v1"))
}
fn disk_path(key: &str) -> Option<PathBuf> {
    let digest = Sha256::digest(key.as_bytes());
    root().map(|dir| dir.join(format!("{}.json", hex::encode(digest))))
}
fn read_disk(key: &str) -> Option<DirectorySnapshot> {
    let path = disk_path(key)?;
    if fs::metadata(&path).ok()?.len() > MAX_SNAPSHOT_BYTES {
        let _ = fs::remove_file(&path);
        return None;
    }
    let bytes = fs::read(&path).ok()?;
    let parsed = serde_json::from_slice::<DirectorySnapshot>(&bytes).ok()?;
    (parsed.key == key).then_some(parsed)
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}
fn insert(state: &mut CacheState, key: &str, value: DirectorySnapshot) {
    if let Some(old) = state.values.remove(key) {
        state.entries = state.entries.saturating_sub(old.listing.entries.len());
    }
    state.recent.retain(|item| item != key);
    state.recent.push_back(key.to_owned());
    state.entries += value.listing.entries.len();
    state.values.insert(key.to_owned(), value);
    while state.values.len() > MAX_MEMORY_DIRECTORIES || state.entries > MAX_MEMORY_ENTRIES {
        let Some(old) = state.recent.pop_front() else {
            break;
        };
        if let Some(removed) = state.values.remove(&old) {
            state.entries = state.entries.saturating_sub(removed.listing.entries.len());
        }
    }
}
fn persist(snapshot: &DirectorySnapshot) {
    let Some(path) = disk_path(&snapshot.key) else {
        return;
    };
    let Ok(bytes) = serde_json::to_vec(snapshot) else {
        return;
    };
    if bytes.len() as u64 > MAX_SNAPSHOT_BYTES {
        return;
    }
    let Some(parent) = path.parent() else { return };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    // NamedTempFile supports cross-platform replacement and prevents colliding
    // snapshot writers from sharing a fixed ".tmp" path.
    use std::io::Write;
    if let Ok(mut temp) = tempfile::NamedTempFile::new_in(parent) {
        if temp.write_all(&bytes).is_ok() && temp.as_file().sync_all().is_ok() {
            let _ = temp.persist(&path);
        }
    }
    static WRITES: AtomicUsize = AtomicUsize::new(0);
    if WRITES.fetch_add(1, Ordering::Relaxed) % 128 == 0 {
        std::thread::spawn(prune_disk);
    }
}
fn prune_disk() {
    let Some(root) = root() else { return };
    let Ok(files) = fs::read_dir(root) else {
        return;
    };
    let mut files = files
        .filter_map(Result::ok)
        .filter(|f| f.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|f| {
            let metadata = f.metadata().ok()?;
            Some((metadata.modified().ok(), f.path(), metadata.len()))
        })
        .collect::<Vec<_>>();
    files.sort_by_key(|(when, _, _)| *when);
    let mut total: u64 = files.iter().map(|(_, _, size)| *size).sum();
    let mut count = files.len();
    for (_, path, size) in files {
        if count <= MAX_DISK_FILES && total <= MAX_DISK_BYTES { break; }
        if fs::remove_file(&path).is_ok() {
            count -= 1;
            total = total.saturating_sub(size);
        }
    }
}
pub fn contains(key: &str) -> bool {
    cache().lock().is_ok_and(|c| c.values.contains_key(key))
        || disk_path(key).is_some_and(|p| p.exists())
}
pub fn get(key: &str) -> Option<DirectorySnapshot> {
    {
        let mut state = cache().lock().ok()?;
        if let Some(value) = state.values.get(key).cloned() {
            state.recent.retain(|item| item != key);
            state.recent.push_back(key.to_owned());
            return Some(value);
        }
    }
    let loaded = read_disk(key)?;
    if let Ok(mut state) = cache().lock() {
        insert(&mut state, key, loaded.clone());
    }
    Some(loaded)
}
pub fn store(key: &str, listing: &Listing) {
    let previous = get(key);
    let mut decorations = previous
        .as_ref()
        .map(|p| p.decorations.clone())
        .unwrap_or_default();
    let previous_entries = previous.as_ref().map(|p| {
        p.listing.entries.iter().map(|entry| (&entry.path, get_entry_key(entry)))
            .collect::<HashMap<_, _>>()
    }).unwrap_or_default();
    let refreshed_entries = listing.entries.iter()
        .map(|entry| (&entry.path, get_entry_key(entry)))
        .collect::<HashMap<_, _>>();
    decorations.retain(|path, _| {
        previous_entries.get(path) == refreshed_entries.get(path)
            && previous_entries.contains_key(path)
    });
    // Only stable metadata identities are preserved. The resource service may
    // explicitly replace decorations after its current resolver pass.
    let snapshot = DirectorySnapshot {
        key: key.into(),
        listing: listing.clone(),
        decorations,
        captured_at: now(),
    };
    if let Ok(mut state) = cache().lock() {
        insert(&mut state, key, snapshot.clone());
    }
    persist(&snapshot);
}
// Decoration entries are tied to their exact metadata revision when published.
fn get_entry_key(entry: &Entry) -> String {
    serde_json::to_string(entry).unwrap_or_default()
}
pub fn set_decorations(key: &str, presented: &[Entry], values: Vec<Decoration>) {
    let Some(mut snapshot) = get(key) else { return };
    let existing = snapshot.listing.entries.iter()
        .map(|entry| (&entry.path, get_entry_key(entry))).collect::<HashMap<_, _>>();
    let resolved = values.into_iter()
        .map(|decoration| (decoration.path.clone(), decoration))
        .collect::<HashMap<_, _>>();
    for entry in presented {
        // Do not attach a stale decoration to a resource changed in the meantime.
        if existing.get(&entry.path) != Some(&get_entry_key(entry)) { continue; }
        if let Some(decoration) = resolved.get(&entry.path) {
            snapshot.decorations.insert(entry.path.clone(), decoration.clone());
        } else {
            snapshot.decorations.remove(&entry.path);
        }
    }
    if let Ok(mut state) = cache().lock() {
        insert(&mut state, key, snapshot.clone());
    }
    persist(&snapshot);
}
pub fn mutate(key: &str, update: impl FnOnce(&mut Listing)) {
    let Some(mut snapshot) = get(key) else { return };
    update(&mut snapshot.listing);
    snapshot
        .decorations
        .retain(|path, _| snapshot.listing.entries.iter().any(|e| &e.path == path));
    snapshot.captured_at = now();
    if let Ok(mut state) = cache().lock() {
        insert(&mut state, key, snapshot.clone());
    }
    persist(&snapshot);
}
pub fn remove_if(predicate: impl Fn(&str) -> bool) {
    if let Ok(mut state) = cache().lock() {
        state.values.retain(|key, _| !predicate(key));
        state.recent.retain(|key| !predicate(key));
        state.entries = state.values.values().map(|v| v.listing.entries.len()).sum();
    }
    if let Some(root) = root() {
        if let Ok(files) = fs::read_dir(root) {
            for file in files.flatten() {
                if let Ok(bytes) = fs::read(file.path()) {
                    if let Ok(snapshot) = serde_json::from_slice::<DirectorySnapshot>(&bytes) {
                        if predicate(&snapshot.key) {
                            let _ = fs::remove_file(file.path());
                        }
                    }
                }
            }
        }
    }
}
