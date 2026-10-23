//! Recently opened files, persisted so the sidebar's Recent location survives
//! restarts. Only local, regular files are tracked: remote downloads and disk
//! image caches have no meaningful path for the user to return to.
use crate::files::{entry, Entry};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

const LIMIT: usize = 40;

pub struct Recent(Mutex<Vec<String>>);

impl Default for Recent {
    fn default() -> Self {
        Self(Mutex::new(load()))
    }
}

fn store() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".fiex").join("recent.json"))
}

fn load() -> Vec<String> {
    store()
        .and_then(|path| fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn save(list: &[String]) {
    let Some(file) = store() else { return };
    if let Some(parent) = file.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(bytes) = serde_json::to_vec(list) {
        let _ = fs::write(file, bytes);
    }
}

pub fn record(state: &Recent, path: &str) {
    if path.starts_with("remote://") || path.contains("://") || path.starts_with("recent://") {
        return;
    }
    if !Path::new(path).is_file() {
        return;
    }
    let mut list = state.0.lock().unwrap();
    list.retain(|item| item != path);
    list.insert(0, path.to_string());
    list.truncate(LIMIT);
    save(&list);
}

pub fn entries(state: &Recent) -> Vec<Entry> {
    state
        .0
        .lock()
        .unwrap()
        .iter()
        .filter_map(|path| entry(Path::new(path)).ok())
        .filter(|entry| !entry.is_dir)
        .collect()
}
