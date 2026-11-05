use crate::trash_path;
use std::{
    collections::HashSet,
    path::Path,
    sync::{Mutex, OnceLock},
};

#[derive(serde::Serialize, serde::Deserialize)]
struct TrashOrigin {
    name: String,
    original: String,
}

static MUTATION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn lock() -> &'static Mutex<()> {
    MUTATION_LOCK.get_or_init(|| Mutex::new(()))
}

pub(crate) fn with_lock<T>(operation: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    let _guard = lock().lock().unwrap_or_else(|error| error.into_inner());
    operation()
}

fn journal_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|home| home.join(".fiex").join("trash-origins.json"))
}

fn load() -> Vec<TrashOrigin> {
    journal_path()
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn save(entries: &[TrashOrigin]) {
    let Some(path) = journal_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(bytes) = serde_json::to_vec(entries) {
        let _ = std::fs::write(path, bytes);
    }
}

fn names() -> HashSet<String> {
    trash_path()
        .and_then(|path| std::fs::read_dir(path).ok())
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect()
}

fn record(original: &str, before: &HashSet<String>) {
    let after = names();
    let added: Vec<_> = after.difference(before).collect();
    // Never guess when another process concurrently puts an item in the Trash.
    if added.len() != 1 {
        return;
    }
    let name = added[0].clone();
    let mut entries = load();
    entries.retain(|entry| entry.name != name && after.contains(&entry.name));
    entries.push(TrashOrigin {
        name,
        original: original.into(),
    });
    save(&entries);
}

pub(super) fn original(name: &str) -> Option<String> {
    load()
        .into_iter()
        .find(|entry| entry.name == name)
        .map(|entry| entry.original)
}

pub(super) fn forget_locked(name: &str) {
    let mut entries = load();
    let old_len = entries.len();
    entries.retain(|entry| entry.name != name);
    if entries.len() != old_len {
        save(&entries);
    }
}

pub(super) fn clear_locked() {
    save(&[]);
}

pub(crate) fn move_to_trash(path: &Path) -> Result<(), String> {
    with_lock(|| {
        let before = names();
        fiex_platform::move_to_trash(path)?;
        record(&path.to_string_lossy(), &before);
        Ok(())
    })
}
