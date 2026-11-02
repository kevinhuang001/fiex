use super::*;

/// URI-backed resources have no host path. Never canonicalize or materialize
/// these identifiers as files, even for metadata-only extension calls.
pub fn opaque(path: &str) -> bool {
    provider_id(path).is_some()
}

pub fn resource_entry(path: &str) -> Result<Entry, String> {
    stat(path, "core:resource-metadata")
}

pub fn icon_for(path: &str) -> Option<String> {
    if provider_id(path) != Some("core:applications") || applications::is_user(path) {
        return None;
    }
    let entry = applications::system_entry(path).ok()?;
    applications::cached_icon(entry.link_target.as_deref()?)
}

pub fn default_view(path: &str) -> Option<&'static str> {
    (provider_id(path) == Some("core:applications")).then_some("icons")
}

/// A Recent alias can be revealed only when its original resource still exists
/// on a local filesystem. Remote, USBFS and plugin-mount sources are excluded.
pub(crate) fn recent_reveal_source(alias: &str) -> Option<String> {
    let source = crate::core::recent::source_for(alias).ok()?;
    revealable_source(&source).then_some(source)
}
