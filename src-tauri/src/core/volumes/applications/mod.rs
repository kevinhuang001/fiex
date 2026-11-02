//! The "Applications" virtual folder. Core enumerates installed applications
//! itself so opening the entry never has to launch Finder or Explorer.

pub mod folders;
mod icons;
mod launch;
mod platform;
mod store;
use crate::core::files::Entry;
use crate::core::fs::directory_cache;
pub use icons::cached_icon;
#[cfg(any(target_os = "windows", test))]
pub(crate) use launch::is_executable_target;
pub use launch::open;
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
use store::list_user;
pub use store::{
    after_rename, candidate_path, create_stored, is_user, list_at, remove_user, rename_stored,
    stored_entry, stored_path, stored_uri, validate_child,
};
#[cfg(test)]
pub(super) use store::{folder_listing, user_encoded};

pub const URI: &str = "applications://";
/// User-created application-store nodes live directly under the view root.
/// The `~` marker keeps them distinct from system application identities,
/// which are base64-encoded host paths.
pub const USER_PREFIX: &str = "applications://~";

fn entry(name: String, path: String, extension: &str, modified: u64) -> Entry {
    Entry {
        path,
        name,
        is_dir: false,
        is_symlink: false,
        size: 0,
        modified,
        created: modified,
        extension: extension.into(),
        mime: "application/x-fiex-application".into(),
        link_target: None,
        allowed_ops: vec!["stat".into()],
        tags: vec![],
        has_cache: false,
        pinned: false,
        uploaded: false,
        sync: None,
    }
}

pub fn list() -> Result<Vec<Entry>, String> {
    let system = refresh_system_entries()?;
    let mapped = folders::mapped_apps();
    Ok(root_listing(system, list_user(), &mapped))
}

fn root_listing(system: Vec<Entry>, user: Vec<Entry>, mapped: &HashSet<String>) -> Vec<Entry> {
    let mut entries: Vec<Entry> = system
        .into_iter()
        .filter(|entry| !mapped.contains(&entry.path))
        .collect();
    entries.extend(user);
    entries
}

/// System applications live in the platform's installed-app namespace, not in
/// a real directory. Keep their last enumeration so a folder listing does not
/// re-run the platform query after a move.
fn system_cache() -> &'static Mutex<Option<Vec<Entry>>> {
    static CACHE: OnceLock<Mutex<Option<Vec<Entry>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

fn cached_system_entries() -> Option<Vec<Entry>> {
    system_cache()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
}

fn store_system_entries(entries: &[Entry]) {
    *system_cache()
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = Some(entries.to_vec());
}

fn refresh_system_entries() -> Result<Vec<Entry>, String> {
    let entries = platform::platform_list()?;
    store_system_entries(&entries);
    let ids = entries
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<Vec<_>>();
    folders::refresh(&ids);
    Ok(entries)
}

/// All installed applications, including those currently mapped into folders.
pub fn system_entries() -> Result<Vec<Entry>, String> {
    if let Some(entries) = cached_system_entries() {
        return Ok(entries);
    }
    let entries = platform::platform_list()?;
    store_system_entries(&entries);
    Ok(entries)
}

pub fn is_system_app(path: &str) -> bool {
    path != URI && path.starts_with(URI) && !is_user(path)
}

pub fn is_mapped(path: &str) -> bool {
    folders::is_mapped(path)
}

pub fn folder_for(path: &str) -> Option<String> {
    folders::folder_for(path)
}

pub fn system_entry(path: &str) -> Result<Entry, String> {
    if !is_system_app(path) {
        return Err("not a system application".into());
    }
    system_entries()?
        .into_iter()
        .find(|entry| entry.path == path)
        .ok_or_else(|| crate::i18n::t("应用不存在").to_string())
}

/// Move system applications into a folder (or back to the root). User-store
/// files use the regular filesystem transfer instead.
pub fn move_system_apps(paths: &[String], destination: &str) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut entries = Vec::with_capacity(paths.len());
    let mut previous = Vec::with_capacity(paths.len());
    for path in paths {
        if !is_system_app(path) {
            return Err("not a system application".into());
        }
        entries.push(system_entry(path)?);
        previous.push(folders::folder_for(path));
    }
    folders::move_apps(paths, destination)?;
    for ((app, entry), previous) in paths.iter().zip(entries).zip(previous) {
        directory_cache::remove_entry(URI, app);
        if let Some(folder) = previous {
            directory_cache::remove_entry(&folder, app);
        }
        if destination == URI {
            directory_cache::add_entry(URI, entry);
        } else {
            directory_cache::add_entry(destination, entry);
        }
    }
    Ok(())
}

/// Context-menu delete for an application entry. Mapped system apps return to
/// the root; user-store files are removed through the normal store rules.
pub fn remove_entries(paths: &[String]) -> Result<(), String> {
    // Review the whole selection before any mutation; mixed selections cannot
    // accidentally remove user data before hitting a protected system app.
    if paths.iter().any(|path| !is_user(path)) {
        return Err(crate::i18n::t("系统应用不能在 Fiex 中删除").into());
    }
    for path in paths {
        remove_user(path)?;
    }
    Ok(())
}

/// Move user-store entries to the platform Trash. Mapped system apps in a
/// trashed folder return to the root.
pub fn trash_entries(paths: &[String]) -> Result<(), String> {
    // Review the whole selection before any mutation; mixed selections cannot
    // accidentally remove user data before hitting a protected system app.
    if paths.iter().any(|path| !is_user(path)) {
        return Err(crate::i18n::t("系统应用不能在 Fiex 中删除").into());
    }
    for path in paths {
        trash_user(path)?;
    }
    Ok(())
}

fn trash_user(uri: &str) -> Result<(), String> {
    let target = stored_path(uri)?;
    let parent = target.parent().ok_or("application item has no parent")?;
    let root = stored_path(URI)?;
    let parent_uri = if parent == root {
        URI.to_string()
    } else {
        stored_uri(parent)?
    };
    let directory = target.is_dir();
    let path = target.to_string_lossy().into_owned();
    let result = crate::core::operations::recycle(std::slice::from_ref(&path))?;
    if let Some(error) = result.errors.into_iter().next() {
        return Err(error);
    }
    directory_cache::remove_entry(&parent_uri, uri);
    directory_cache::remove_entry(URI, uri);
    if directory {
        let returned = folders::forget_folder(uri)?;
        directory_cache::drop_subtree(uri);
        for app in returned {
            if let Ok(entry) = system_entry(&app) {
                directory_cache::add_entry(URI, entry);
            }
        }
    }
    Ok(())
}

fn encode(value: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(value)
}

fn decode(value: &str) -> Result<String, String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|error| error.to_string())?;
    String::from_utf8(bytes).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests;
