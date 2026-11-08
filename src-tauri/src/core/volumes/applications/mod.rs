//! Core adapter between the Applications module and OS-specific app handling.

mod launch;
mod platform;
mod state;
mod system;
use crate::core::files::Entry;
pub use system::{system_entries, system_entry};
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
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};

pub const URI: &str = fiex_applications::URI;

fn catalog() -> &'static Arc<fiex_applications::ApplicationCatalog> {
    static CATALOG: OnceLock<Arc<fiex_applications::ApplicationCatalog>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        Arc::new(fiex_applications::ApplicationCatalog::new(
            fiex_applications::ApplicationsStore::new(state::storage()),
            platform::backend(),
        ))
    })
}

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
    state::ensure_ready()?;
    list_at(URI)
}

pub fn list_at(uri: &str) -> Result<Vec<Entry>, String> {
    let cached = system_entries()?;
    let mut entries = state::folder_children(uri)?;
    for entry in &mut entries {
        if let Some(system) = cached.iter().find(|system| system.path == entry.path) {
            *entry = system.clone();
        }
    }
    Ok(entries)
}

pub fn decoration_info(
    paths: &[String],
) -> HashMap<String, fiex_applications::ApplicationDecorationInfo> {
    catalog().decoration_info(paths)
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
    platform::backend().classify_host_application(std::path::Path::new(path), target)
}

pub fn validate_import_asset(path: &std::path::Path, display_name: &str) -> Result<(), String> {
    platform::backend().validate_import_asset(path, display_name)
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
