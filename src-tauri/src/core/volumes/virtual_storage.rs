//! Storage implementations for Core-owned virtual views. Presentation and
//! dedicated actions stay in `virtual_resolvers`; adapters never mix them.
#[cfg(not(target_os = "windows"))]
use super::trash;
use super::{applications, recycle, virtual_resolvers};
#[cfg(not(target_os = "windows"))]
use crate::core::files;
use crate::core::files::Entry;
use crate::core::recent;
#[cfg(not(target_os = "windows"))]
use std::path::Path;

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
        "core:applications" => applications::list()?
            .into_iter()
            .find(|entry| entry.path == path)
            .ok_or_else(|| crate::i18n::t("应用不存在").into()),
        "core:trash" => trash_entry(path),
        _ => Err("Unsupported: builtin view is not a storage node".into()),
    }
}

pub fn list(path: &str) -> Result<Vec<Entry>, String> {
    if path == virtual_resolvers::RECENT_URI {
        return Ok(recent::entries(&recent::Recent::default()));
    }
    if path == applications::URI || applications::is_user(path) {
        return applications::list_at(path);
    }
    let view = virtual_resolvers::view_for(path).ok_or("unknown builtin VFS")?;
    match view.id() {
        "core:trash" => trash_list(path),
        _ => Err("Unsupported: builtin view cannot be listed".into()),
    }
}

fn trash_list(path: &str) -> Result<Vec<Entry>, String> {
    let _ = path;
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

fn trash_entry(path: &str) -> Result<Entry, String> {
    #[cfg(target_os = "windows")]
    {
        let _ = path;
        recycle::list()?
            .into_iter()
            .find(|entry| entry.path == path)
            .ok_or_else(|| crate::i18n::t("回收站项目无效").into())
    }
    #[cfg(not(target_os = "windows"))]
    {
        files::entry(Path::new(path))
    }
}
