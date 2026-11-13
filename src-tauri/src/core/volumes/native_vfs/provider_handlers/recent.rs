use crate::core::files::Entry;
use crate::core::volumes::native_vfs::{NativeVfsBehavior, NativeVfsIdentity, RECENT_URI};

fn recent_identity(_path: &str, provider_id: &'static str) -> Option<NativeVfsIdentity> {
    Some(NativeVfsIdentity {
        provider_id,
        storage_type: None,
        mount_id: provider_id.into(),
        available: true,
        host_path: false,
    })
}

fn revalidate_recent_listing(entries: &mut Vec<Entry>) -> Result<(), String> {
    crate::core::volumes::native_vfs::filter_recent_cached_entries(entries)
}

#[cfg(feature = "desktop")]
fn resolve_recent_navigation_source(alias: &str) -> Option<String> {
    crate::core::volumes::native_vfs::recent_source_for(alias).ok()
}

fn recent_is_tree_path(path: &str) -> bool {
    path == RECENT_URI
}

fn recent_resource_kind(path: &str) -> &'static str {
    if path == RECENT_URI {
        "directory"
    } else {
        "file"
    }
}

fn validate_recent_child(parent: &str, child: &str) -> Result<(), String> {
    if parent != RECENT_URI {
        return Err("recent view has no writable child identities".into());
    }
    crate::core::volumes::native_vfs::recent_source_for(child).map(|_| ())
}

pub(crate) struct RecentOperations;

impl NativeVfsBehavior for RecentOperations {
    fn validate_child(&self, parent: &str, child: &str) -> Option<Result<(), String>> {
        Some(validate_recent_child(parent, child))
    }

    fn is_tree_path(&self, path: &str) -> bool {
        recent_is_tree_path(path)
    }

    fn resource_kind(&self, path: &str) -> Option<&'static str> {
        Some(recent_resource_kind(path))
    }

    fn provider_identity(
        &self,
        path: &str,
        provider_id: &'static str,
    ) -> Option<NativeVfsIdentity> {
        recent_identity(path, provider_id)
    }

    #[cfg(feature = "desktop")]
    fn navigation_source(&self, path: &str) -> Option<String> {
        resolve_recent_navigation_source(path)
    }

    fn revalidate_cached_listing(&self, entries: &mut Vec<Entry>) -> Option<Result<(), String>> {
        Some(revalidate_recent_listing(entries))
    }
}
