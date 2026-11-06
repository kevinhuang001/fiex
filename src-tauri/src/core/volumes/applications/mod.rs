//! The "Applications" virtual folder. Core enumerates installed applications
//! itself so opening the entry never has to launch Finder or Explorer.

mod launch;
mod platform;
mod state;
use crate::core::files::Entry;
#[cfg(feature = "desktop")]
pub fn cached_icon(source: &str) -> Option<String> {
    fiex_applications::cached_icon_with(
        source,
        |source, target| {
            image::open(source)
                .map_err(|error| error.to_string())?
                .thumbnail(128, 128)
                .save(target)
                .map_err(|error| error.to_string())
        },
        |source, target| {
            #[cfg(target_os = "windows")]
            {
                crate::core::volumes::windows_shell::extract_icon(source, target)
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = (source, target);
                Err("native Shell icons are unavailable on this platform".into())
            }
        },
        |source, target| {
            let icon = file_icon_provider::get_file_icon(source, 128)
                .map_err(|error| error.to_string())?;
            image::RgbaImage::from_raw(icon.width, icon.height, icon.pixels)
                .ok_or_else(|| "icon pixel buffer is invalid".to_owned())?
                .save(target)
                .map_err(|error| error.to_string())
        },
    )
}
#[cfg(not(feature = "desktop"))]
pub fn cached_icon(_source: &str) -> Option<String> {
    None
}
#[cfg(target_os = "windows")]
pub(crate) use launch::is_executable_target;
pub use launch::open;
pub use state::{PendingKind, PendingTarget};
use std::sync::{Condvar, Mutex, OnceLock};

pub const URI: &str = fiex_applications::URI;

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
        storage_projection: Default::default(),
        sync: None,
    }
}

pub fn list() -> Result<Vec<Entry>, String> {
    refresh_system_entries()?;
    list_at(URI)
}

pub fn list_at(uri: &str) -> Result<Vec<Entry>, String> {
    system_entries()?;
    let mut entries = state::folder_children(uri)?;
    let cached = cached_system_entries().unwrap_or_default();
    for entry in &mut entries {
        if let Some(system) = cached.iter().find(|system| system.path == entry.path) {
            *entry = system.clone();
        }
    }
    Ok(entries)
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

#[derive(Default)]
struct EnumerationState {
    running: bool,
    last_result: Option<Result<Vec<Entry>, String>>,
}

fn enumeration_state() -> &'static (Mutex<EnumerationState>, Condvar) {
    static STATE: OnceLock<(Mutex<EnumerationState>, Condvar)> = OnceLock::new();
    STATE.get_or_init(|| (Mutex::new(EnumerationState::default()), Condvar::new()))
}

fn refresh_system_entries() -> Result<Vec<Entry>, String> {
    let (state, completed) = enumeration_state();
    let mut current = state.lock().unwrap_or_else(|error| error.into_inner());
    if current.running {
        while current.running {
            current = completed
                .wait(current)
                .unwrap_or_else(|error| error.into_inner());
        }
        return current
            .last_result
            .clone()
            .unwrap_or_else(|| Err("application enumeration did not produce a result".into()));
    }
    current.running = true;
    drop(current);

    let result = enumerate_system_entries();
    let mut current = state.lock().unwrap_or_else(|error| error.into_inner());
    current.last_result = Some(result.clone());
    current.running = false;
    completed.notify_all();
    result
}

fn enumerate_system_entries() -> Result<Vec<Entry>, String> {
    let mut entries = platform::platform_list()?;
    let records = entries
        .iter()
        .map(|entry| {
            let encoded = entry
                .path
                .strip_prefix(URI)
                .ok_or("platform application has an invalid identity")?;
            let locator = decode(encoded)?;
            Ok((locator, entry.name.clone()))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let identities = state::register_system_apps(&records)?;
    for entry in &mut entries {
        let encoded = entry
            .path
            .strip_prefix(URI)
            .ok_or("platform application has an invalid identity")?;
        let locator = decode(encoded)?;
        entry.path = identities
            .get(&locator)
            .cloned()
            .ok_or("platform application identity was not persisted")?;
    }
    store_system_entries(&entries);
    Ok(entries)
}

/// All installed applications, including those currently mapped into folders.
pub fn system_entries() -> Result<Vec<Entry>, String> {
    if let Some(entries) = cached_system_entries() {
        return Ok(entries);
    }
    refresh_system_entries()
}

pub fn is_system_app(path: &str) -> bool {
    state::source(path)
        .is_ok_and(|app| matches!(app.source, state::ApplicationSource::System { .. }))
}

/// Whether a stored user node is a platform-verified launch target.
pub fn is_user_application(path: &str) -> bool {
    is_user(path)
        && state::source(path)
            .is_ok_and(|app| matches!(app.source, state::ApplicationSource::Imported { .. }))
}

/// Classify a trusted host resource for Applications transfer policy.
pub(crate) fn classify_host_application(path: &str, target: Option<&str>) -> bool {
    platform::classify_host_application(std::path::Path::new(path), target)
}

pub fn validate_import_asset(path: &std::path::Path, display_name: &str) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("Only regular application assets can be imported".into());
    }
    let extension = std::path::Path::new(display_name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let supported = match std::env::consts::OS {
        "windows" => {
            matches!(extension.as_str(), "exe" | "lnk")
                && (extension == "lnk"
                    || crate::core::volumes::application_policy::executable_file(
                        &path.to_string_lossy(),
                    ))
        }
        "linux" => {
            matches!(extension.as_str(), "desktop" | "appimage")
                && classify_host_application(&path.to_string_lossy(), None)
        }
        "macos" => false,
        _ => false,
    };
    if supported {
        Ok(())
    } else {
        Err("The file is not a supported application asset".into())
    }
}

pub fn system_entry(path: &str) -> Result<Entry, String> {
    let node = state::source(path)?;
    if !matches!(node.source, state::ApplicationSource::System { .. }) {
        return Err("not a system application".into());
    }
    match system_entries() {
        Ok(entries) => {
            if let Some(entry) = entries.iter().find(|entry| entry.path == path) {
                return Ok(entry.clone());
            }
            if entries.iter().any(|entry| {
                state::app_id(&entry.path) == state::app_id(path) && state::app_id(path).is_some()
            }) {
                return state::entry(path);
            }
        }
        Err(error) if !node.unavailable => return Err(error),
        Err(_) => {}
    }
    if node.unavailable {
        return state::entry(path);
    }
    Err(crate::i18n::t("应用不存在").to_string())
}

pub fn is_user(path: &str) -> bool {
    state::is_user_app(path)
}

pub fn is_folder(path: &str) -> bool {
    state::is_folder(path)
}

pub fn is_app(path: &str) -> bool {
    state::is_app(path)
}

pub fn entry_for_uri(path: &str) -> Result<Entry, String> {
    state::entry(path)
}

pub fn create_virtual_folder(parent: &str, name: &str) -> Result<Entry, String> {
    state::create_folder(parent, name)
}

pub fn rename_virtual_folder(path: &str, name: &str) -> Result<Entry, String> {
    state::rename_folder(path, name)
}

pub fn remove_virtual_folder(path: &str) -> Result<(), String> {
    state::remove_empty_folder(path)
}

pub fn validate_remove(path: &str) -> Result<(), String> {
    state::validate_remove(path)
}

pub fn parent_uri(path: &str) -> Result<String, String> {
    state::parent_uri(path)
}

pub fn validate_child(parent: &str, child: &str) -> Result<(), String> {
    state::validate_child(parent, child)
}

pub fn reserve_child(parent: &str, name: &str, directory: bool) -> Result<String, String> {
    state::reserve_child(
        parent,
        name,
        if directory {
            state::PendingKind::Folder
        } else {
            state::PendingKind::Application
        },
    )
}

pub fn reserved_child(parent: &str, name: &str) -> Result<state::PendingTarget, String> {
    state::reserved_child(parent, name)
}

pub fn pending_target(path: &str) -> Option<state::PendingTarget> {
    state::pending_target(path)
}

pub fn cancel_reservation(path: &str) {
    state::cancel_reservation(path)
}

pub fn commit_reserved_folder(path: &str) -> Result<Entry, String> {
    state::commit_reserved_folder(path)
}

pub fn commit_reserved_application(path: &str) -> Result<Entry, String> {
    state::commit_reserved_application(path)
}

pub fn rename_app(path: &str, name: &str) -> Result<Entry, String> {
    state::rename_imported(path, name)
}

pub fn rename_node(path: &str, parent: &str, name: &str) -> Result<Entry, String> {
    state::rename_node(path, parent, name)
}

pub fn remove_node(path: &str) -> Result<(), String> {
    state::remove_node(path)
}

pub fn move_nodes(paths: &[String], destination: &str) -> Result<Vec<String>, String> {
    state::move_nodes(paths, destination)
}

pub fn asset_path(path: &str) -> Result<std::path::PathBuf, String> {
    state::asset_path(path)
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
