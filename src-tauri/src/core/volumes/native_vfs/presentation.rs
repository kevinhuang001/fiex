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

/// Check a persisted locator against its current provider and parent rules.
pub(crate) fn navigable_source(source: &str) -> Option<String> {
    let resource = crate::core::fs::trusted(source, "core:recent-navigation").ok()?;
    let entry = resource.desktop_entry().ok()?;
    if entry.is_dir || crate::core::fs::parent_path(source).is_err() {
        return None;
    }
    Some(source.to_owned())
}

/// A Recent alias can navigate only when Core can still stat its source and
/// resolve that source's provider-owned parent.
pub(crate) fn recent_navigation_source(alias: &str) -> Option<String> {
    let source = crate::core::recent::source_for(alias).ok()?;
    navigable_source(&source)
}
