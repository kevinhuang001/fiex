//! Core projection adapter for the Trash module's opaque path identities.
use crate::core::files::Entry;
use fiex_provider_contracts::{FsEntry, FsListing};
use std::path::PathBuf;

pub const URI: &str = fiex_trash::TRASH_URI;

fn backend() -> &'static fiex_trash::TrashDirectory {
    fiex_trash::system_trash_directory()
}

fn project(entry: FsEntry) -> Entry {
    crate::core::volumes::native_vfs::core_entry(entry)
}

fn project_listing(listing: FsListing) -> Vec<Entry> {
    listing.entries.into_iter().map(project).collect()
}

fn map_error(error: String) -> String {
    if error.starts_with("NotFound: cannot locate the system Trash") {
        return crate::i18n::t("无法定位废纸篓").into();
    }
    #[cfg(target_os = "macos")]
    if error.starts_with("PermissionDenied:") {
        return format!(
            "PermissionDenied: {}",
            crate::i18n::t("Fiex 无权访问废纸篓。请打开“系统设置 → 隐私与安全性 → 完全磁盘访问权限”，添加并开启 Fiex，然后退出并重新打开 Fiex，再访问废纸篓。")
        );
    }
    error
}

pub fn handles(path: &str) -> bool {
    backend().handles(path)
}

/// Check access before painting cached metadata; revoked access cannot revive
/// entries kept in a directory snapshot.
pub fn check_access() -> Result<(), String> {
    backend().check_access().map_err(map_error)
}

pub fn physical(uri: &str) -> Result<PathBuf, String> {
    backend().physical(uri).map_err(map_error)
}

pub fn action_path(path: &str) -> Result<String, String> {
    backend().action_path(path).map_err(map_error)
}

pub fn list(uri: &str) -> Result<Vec<Entry>, String> {
    backend().list(uri).map(project_listing).map_err(map_error)
}

pub fn validate_child(parent: &str, child: &str) -> Result<(), String> {
    backend().validate_child(parent, child).map_err(map_error)
}

pub fn purge(uri: &str) -> Result<(), String> {
    backend().purge(uri).map_err(map_error)
}

pub fn entry(uri: &str) -> Result<Entry, String> {
    let mut entry = backend().entry(uri).map(project).map_err(map_error)?;
    if uri == URI {
        entry.name = crate::i18n::t("废纸篓").into();
    }
    Ok(entry)
}

pub fn is_tree(path: &str) -> bool {
    backend().is_tree(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trash_roots_resolve_through_the_builtin_module() {
        assert!(handles(URI));
        assert!(is_tree(URI));
        assert!(is_tree(fiex_trash::RECYCLE_BIN_URI));
        assert!(!handles(fiex_trash::RECYCLE_BIN_URI));
    }
}
