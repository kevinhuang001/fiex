use super::{Decoration, Listing};
use std::collections::HashMap;

#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct Snapshot {
    pub(super) listing: Listing,
    pub(super) decorations: Vec<Decoration>,
    /// Paths whose presentation providers have been queried, including paths
    /// for which no icon or badge was produced.
    #[serde(default)]
    pub(super) resolved_paths: Vec<String>,
    pub(super) stamp: String,
}

fn decoration_index(decorations: &[Decoration]) -> HashMap<&str, Vec<&Decoration>> {
    let mut index: HashMap<&str, Vec<&Decoration>> = HashMap::new();
    for decoration in decorations {
        index
            .entry(decoration.path.as_str())
            .or_default()
            .push(decoration);
    }
    index
}

pub(super) fn same_entry(
    left: &crate::core::files::Entry,
    right: &crate::core::files::Entry,
) -> bool {
    left.path == right.path
        && left.name == right.name
        && left.is_dir == right.is_dir
        && left.is_symlink == right.is_symlink
        && left.size == right.size
        && left.modified == right.modified
        && left.created == right.created
        && left.extension == right.extension
        && left.mime == right.mime
        && left.link_target == right.link_target
        && left.allowed_ops == right.allowed_ops
        && left.tags.len() == right.tags.len()
        && left
            .tags
            .iter()
            .zip(&right.tags)
            .all(|(a, b)| a.name == b.name && a.color == b.color)
        && left.has_cache == right.has_cache
        && left.pinned == right.pinned
        && left.uploaded == right.uploaded
        && left.sync == right.sync
}

pub(super) fn listing_index(listing: &Listing) -> HashMap<&str, &crate::core::files::Entry> {
    listing
        .entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect()
}

fn same_listing(previous: &Snapshot, listing: &Listing) -> bool {
    if previous.listing.entries.len() != listing.entries.len() {
        return false;
    }
    let old = listing_index(&previous.listing);
    listing.entries.iter().all(|entry| {
        old.get(entry.path.as_str())
            .is_some_and(|previous| same_entry(previous, entry))
    })
}

pub(super) fn reuse_from_snapshot(
    previous: &Snapshot,
    listing: &Listing,
) -> (Vec<Decoration>, bool) {
    let exact = same_listing(previous, listing);
    if exact {
        return (previous.decorations.clone(), true);
    }
    let old_entries = listing_index(&previous.listing);
    let old_decorations = decoration_index(&previous.decorations);
    let mut reusable = Vec::new();
    for entry in &listing.entries {
        if !old_entries
            .get(entry.path.as_str())
            .is_some_and(|previous| same_entry(previous, entry))
        {
            continue;
        }
        if let Some(values) = old_decorations.get(entry.path.as_str()) {
            reusable.extend(values.iter().map(|value| (*value).clone()));
        }
    }
    (reusable, false)
}
