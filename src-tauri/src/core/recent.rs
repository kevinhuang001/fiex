//! Recently opened files, persisted so the sidebar's Recent location survives
//! restarts. Store source identities, never downloaded cache paths.
use crate::core::files::{entry, Entry};
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
    if crate::core::volumes::virtual_resolvers::opaque(path) {
        return;
    }
    if crate::plugins::resource::resource_context::ResourceSnapshot::resolve(path).map_or(
        true,
        |resource| {
            resource.is_dir
                || (!crate::plugins::mounts::handles(path)
                    && crate::usbfs::route(path).is_none()
                    && resource.stat.is_none())
        },
    ) {
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
        .filter_map(|path| {
            if crate::remote::is_remote(path) {
                crate::remote::stat(path)
            } else if crate::plugins::vfs::handles(path) {
                crate::plugins::vfs::stat(path)
            } else if let Some(route) = crate::usbfs::route(path) {
                crate::usbfs::entry(&route)
            } else {
                entry(Path::new(path))
            }
            .ok()
        })
        .filter(|entry| !entry.is_dir)
        .collect()
}
