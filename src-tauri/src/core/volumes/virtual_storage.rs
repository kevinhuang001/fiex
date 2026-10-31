//! Storage implementations for Core-owned virtual views. Presentation and
//! dedicated actions stay in `virtual_resolvers`; adapters never mix them.
//!
//! List always enumerates the source; navigation caching is owned by Core FS.
use super::{applications, virtual_resolvers};
use crate::core::files::Entry;

pub fn entry(path: &str) -> Result<Entry, String> {
    virtual_resolvers::view_for(path)
        .ok_or_else(|| "Unsupported: unknown builtin node".to_string())?
        .stat(path)
}

pub fn list(path: &str) -> Result<Vec<Entry>, String> {
    virtual_resolvers::view_for(path)
        .ok_or_else(|| "Unsupported: builtin node has no children".to_string())?
        .list(path)
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
