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

/// A Recent entry is a non-authorizing view alias. The persisted original
/// identity is kept inside Core, never exposed as an OS path through the view.
pub fn alias_for(source: &str) -> String {
    use base64::Engine;
    let id = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(source);
    format!("recent://{id}")
}
pub fn source_for(alias: &str) -> Result<String, String> {
    use base64::Engine;
    let encoded = alias.strip_prefix("recent://").filter(|s| !s.is_empty())
        .ok_or("not a recent item")?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded).map_err(|_| "invalid recent alias")?;
    let path = String::from_utf8(bytes).map_err(|_| "invalid recent identity")?;
    if !load().contains(&path) {
        return Err("StaleRef: item is no longer in Recent".into());
    }
    Ok(path)
}
fn source_entry(path: &str) -> Result<Entry, String> {
    if crate::remote::is_remote(path) {
        crate::remote::stat(path)
    } else if crate::plugins::vfs::handles(path) {
        crate::plugins::vfs::stat(path)
    } else if let Some(route) = crate::usbfs::route(path) {
        crate::usbfs::entry(&route)
    } else {
        entry(Path::new(path))
    }
}
pub fn entry_for_alias(alias: &str) -> Result<Entry, String> {
    let source = source_for(alias)?;
    let mut item = source_entry(&source)?;
    if item.is_dir { return Err("Unsupported: Recent directories are not supported".into()); }
    let cap = crate::plugins::resource::file_capabilities::for_path(&source);
    item.path = alias.into();
    item.link_target = None;
    item.allowed_ops = if cap.read { vec!["stat".into(), "read".into()] }
        else { vec!["stat".into()] };
    Ok(item)
}
pub fn entries(state: &Recent) -> Vec<Entry> {
    state.0.lock().unwrap().iter().filter_map(|path| {
        entry_for_alias(&alias_for(path)).ok()
    }).collect()
}
