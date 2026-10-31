//! Storage implementations for Core-owned virtual views. Presentation and
//! dedicated actions stay in `virtual_resolvers`; adapters never mix them.
//!
//! List always enumerates the source; navigation caching is owned by Core FS.
use super::{applications, recycle, virtual_resolvers};
use crate::core::files::Entry;
use crate::core::recent;

pub fn entry(path: &str) -> Result<Entry, String> {
    if super::trash_vfs::handles(path) {
        return super::trash_vfs::entry(path);
    }
    if path == virtual_resolvers::RECENT_URI || path == applications::URI || path == recycle::URI {
        let view = virtual_resolvers::view_for(path).ok_or("unknown builtin VFS")?;
        return Ok(Entry {
            path: path.into(),
            name: view.id().into(),
            is_dir: true,
            readonly: view.readonly(),
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
    if applications::is_system_app(path) {
        return applications::system_entry(path);
    }
    if recycle::is_virtual(path) {
        return recycle::list()?
            .into_iter()
            .find(|e| e.path == path)
            .ok_or_else(|| "NotFound: Recycle Bin item missing".into());
    }
    Err("Unsupported: unknown builtin node".into())
}
pub fn list(path: &str) -> Result<Vec<Entry>, String> {
    if path == virtual_resolvers::RECENT_URI {
        Ok(recent::entries(&recent::Recent::default()))
    } else if path == applications::URI {
        applications::list()
    } else if applications::is_user(path) {
        applications::list_at(path)
    } else if super::trash_vfs::handles(path) {
        super::trash_vfs::list(path)
    } else if path == recycle::URI {
        recycle::list()
    } else {
        Err("Unsupported: builtin node has no children".into())
    }
}
pub(crate) fn is_virtual_tree_path(path: &str) -> bool {
    virtual_path_key(path).is_some()
}
pub(crate) fn virtual_path_key(path: &str) -> Option<String> {
    (path == virtual_resolvers::RECENT_URI
        || path == applications::URI
        || applications::is_user(path)
        || super::trash_vfs::is_tree(path))
    .then(|| path.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_view_policy_is_projected_into_root_entry() {
        assert!(entry(virtual_resolvers::RECENT_URI).unwrap().readonly);
        assert!(!entry(applications::URI).unwrap().readonly);
    }
}
