//! Presentation snapshots are created exclusively by the directory view boundary.
#[cfg(any(feature = "desktop", test))]
use super::directory_cache::{enabled, key};
use crate::infrastructure::cache::disk;
#[cfg(any(feature = "desktop", test))]
use crate::{core::files::Listing, plugins::providers::decorations::Decoration};
#[cfg(any(feature = "desktop", test))]
use serde::{Deserialize, Serialize};
#[cfg(any(feature = "desktop", test))]
use std::collections::HashMap;
#[cfg(any(feature = "desktop", test))]
fn stamp() -> String {
    use sha2::{Digest, Sha256};
    let catalog = crate::plugins::host::catalog();
    let versions = catalog
        .iter()
        .map(|p| {
            (
                &p.id,
                &p.version,
                p.enabled,
                p.approved,
                &p.granted_permissions,
            )
        })
        .collect::<Vec<_>>();
    let providers = serde_json::to_vec(&versions).unwrap_or_default();
    let prefs = serde_json::to_vec(&crate::plugins::preferences::snapshot()).unwrap_or_default();
    let mut hash = Sha256::new();
    hash.update(crate::i18n::locale().as_bytes());
    hash.update(&providers);
    hash.update(&prefs);
    hex::encode(hash.finalize())
}

#[cfg(any(feature = "desktop", test))]
#[derive(Serialize, Deserialize)]
struct Snapshot {
    listing: Listing,
    decorations: Vec<Decoration>,
    stamp: String,
}
#[cfg(any(feature = "desktop", test))]
fn identity(path: &str) -> String {
    format!("decoration:{}", key(path))
}

#[cfg(any(feature = "desktop", test))]
fn same_entry(left: &crate::core::files::Entry, right: &crate::core::files::Entry) -> bool {
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

#[cfg(any(feature = "desktop", test))]
fn listing_index(listing: &Listing) -> HashMap<&str, &crate::core::files::Entry> {
    listing
        .entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect()
}

#[cfg(any(feature = "desktop", test))]
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

#[cfg(any(feature = "desktop", test))]
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

#[cfg(any(feature = "desktop", test))]
fn reuse_from_snapshot(previous: &Snapshot, listing: &Listing) -> (Vec<Decoration>, bool) {
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

/// Reuse cached decorations as a display hint. The bool reports whether the
/// entire listing matches, not whether every item has a decoration.
#[cfg(any(feature = "desktop", test))]
pub(crate) fn cached(path: &str, listing: &Listing) -> (Vec<Decoration>, bool) {
    if !enabled(path) {
        return (Vec::new(), false);
    }
    let Some(previous) = disk::load::<Snapshot>(&identity(path)) else {
        return (Vec::new(), false);
    };
    if previous.stamp != stamp() {
        return (Vec::new(), false);
    }
    reuse_from_snapshot(&previous, listing)
}

#[cfg(any(feature = "desktop", test))]
pub(crate) fn read(path: &str, listing: &Listing) -> Option<Vec<Decoration>> {
    let (decorations, exact) = cached(path, listing);
    exact.then_some(decorations)
}
#[cfg(any(feature = "desktop", test))]
pub(crate) fn store(path: &str, listing: &Listing, decorations: &[Decoration]) {
    if enabled(path) {
        disk::store(
            &identity(path),
            &Snapshot {
                listing: listing.clone(),
                decorations: decorations.to_vec(),
                stamp: stamp(),
            },
        );
    }
}
pub(crate) fn invalidate(list_key: &str) {
    disk::remove(&format!("decoration:{list_key}"));
}

#[cfg(feature = "desktop")]
pub(crate) fn resolve(path: &str, listing: &Listing) -> Vec<Decoration> {
    use crate::plugins::providers::decorations::{
        query_with_plugins, DecorationEntry, DecorationTag,
    };
    let original_identity = key(path);
    let previous: Option<Snapshot> = enabled(path).then(|| disk::load(&identity(path))).flatten();
    let current_stamp = stamp();
    let previous = previous.filter(|snapshot| snapshot.stamp == current_stamp);
    let (mut decorations, exact) = previous
        .as_ref()
        .map(|snapshot| reuse_from_snapshot(snapshot, listing))
        .unwrap_or_default();
    if exact {
        return decorations;
    }
    let old_entries = previous
        .as_ref()
        .map(|value| listing_index(&value.listing))
        .unwrap_or_default();
    let mut missing: Vec<DecorationEntry> = Vec::new();
    for entry in &listing.entries {
        let unchanged = old_entries
            .get(entry.path.as_str())
            .is_some_and(|previous| same_entry(previous, entry));
        if !unchanged {
            missing.push(DecorationEntry {
                path: entry.path.clone(),
                name: entry.name.clone(),
                is_dir: entry.is_dir,
                extension: entry.extension.clone(),
                mime: entry.mime.clone(),
                allowed_ops: entry.allowed_ops.clone(),
                tags: entry.tags.iter().map(|tag| DecorationTag {
                    name: tag.name.clone(),
                    color: tag.color,
                    hex: None,
                }).collect(),
                sync: entry.sync.clone(),
                has_cache: entry.has_cache,
                pinned: entry.pinned,
                uploaded: entry.uploaded,
                link_target: entry.link_target.clone(),
            });
        }
    }
    if !missing.is_empty() {
        decorations.extend(query_with_plugins(path, missing, true));
    }
    if original_identity == key(path) && enabled(path) {
        store(path, listing, &decorations);
    }
    decorations
}
