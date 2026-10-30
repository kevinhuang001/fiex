//! Storage implementations for Core-owned virtual views. Presentation and
//! dedicated actions stay in `virtual_resolvers`; adapters never mix them.
//!
//! List always enumerates the source; navigation caching is owned by Core FS.
use super::{applications, recycle, virtual_resolvers};
#[cfg(not(target_os = "windows"))]
use super::trash;
#[cfg(not(target_os = "windows"))]
use crate::core::files;
use crate::core::files::Entry;
use crate::core::recent;
#[cfg(not(target_os = "windows"))]
use std::{path::Path, sync::OnceLock};

/// The sidebar's logical Trash place. The platform root is `recyclebin://` on
/// Windows and a real directory on Unix.
const TRASH_ALIAS: &str = "trash://";
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
    if path == virtual_resolvers::RECENT_URI {
        Ok(recent::entries(&recent::Recent::default()))
    } else if path == applications::URI {
        applications::list()
    } else if applications::is_user(path) {
        applications::list_at(path)
    } else if is_trash_root(path) || path == TRASH_ALIAS || path == recycle::URI {
        trash_list_live()
    } else {
        trash_subdir_live(path)
    }
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

fn root_entries(root: &str) -> Result<Vec<Entry>, String> {
    if let Some(listing) = crate::core::fs::directory_cache::read(root, true, &[])? {
        return Ok(listing.entries);
    }
    list(root)
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
    // avoid a canonicalize syscall per entry while resolving view paths.
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
