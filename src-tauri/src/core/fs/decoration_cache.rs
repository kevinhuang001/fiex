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
    /// Paths whose presentation providers have been queried, including paths
    /// for which no icon or badge was produced. This distinguishes a resolved
    /// empty result from an entry that has never been resolved.
    #[serde(default)]
    resolved_paths: Vec<String>,
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

/// Reuse cached decorations as a display hint. The bool is true only when the
/// listing matches and every entry has completed presentation resolution.
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
    let (decorations, listing_exact) = reuse_from_snapshot(&previous, listing);
    let complete = listing_exact
        && listing.entries.iter().all(|entry| {
            previous
                .resolved_paths
                .iter()
                .any(|path| path == &entry.path)
        });
    (decorations, complete)
}

#[cfg(test)]
pub(crate) fn read(path: &str, listing: &Listing) -> Option<Vec<Decoration>> {
    let (decorations, exact) = cached(path, listing);
    exact.then_some(decorations)
}
#[cfg(test)]
pub(crate) fn store(path: &str, listing: &Listing, decorations: &[Decoration]) {
    if enabled(path) {
        disk::store(
            &identity(path),
            &Snapshot {
                listing: listing.clone(),
                decorations: decorations.to_vec(),
                resolved_paths: listing
                    .entries
                    .iter()
                    .map(|entry| entry.path.clone())
                    .collect(),
                stamp: stamp(),
            },
        );
    }
}
pub(crate) fn invalidate(list_key: &str) {
    disk::remove(&format!("decoration:{list_key}"));
}

#[cfg(feature = "desktop")]
pub(crate) fn resolve_missing(path: &str, listing: &Listing) -> Vec<Decoration> {
    use crate::plugins::providers::decorations::{
        query_with_plugins, DecorationEntry, DecorationTag,
    };
    let original_identity = key(path);
    let previous: Option<Snapshot> = enabled(path).then(|| disk::load(&identity(path))).flatten();
    let current_stamp = stamp();
    let previous = previous.filter(|snapshot| snapshot.stamp == current_stamp);
    let (mut decorations, _) = previous
        .as_ref()
        .map(|snapshot| reuse_from_snapshot(snapshot, listing))
        .unwrap_or_default();
    let previous_resolved = previous
        .as_ref()
        .map(|snapshot| {
            snapshot
                .resolved_paths
                .iter()
                .map(String::as_str)
                .collect::<std::collections::HashSet<_>>()
        })
        .unwrap_or_default();
    let old_entries = previous
        .as_ref()
        .map(|value| listing_index(&value.listing))
        .unwrap_or_default();
    let all_resolved = listing.entries.iter().all(|entry| {
        previous_resolved.contains(entry.path.as_str())
            && old_entries
                .get(entry.path.as_str())
                .is_some_and(|old| same_entry(old, entry))
    });
    if all_resolved {
        if original_identity == key(path) && enabled(path) {
            disk::store(
                &identity(path),
                &Snapshot {
                    listing: listing.clone(),
                    decorations,
                    resolved_paths: listing
                        .entries
                        .iter()
                        .map(|entry| entry.path.clone())
                        .collect(),
                    stamp: current_stamp,
                },
            );
        }
        return Vec::new();
    }
    let mut missing: Vec<DecorationEntry> = Vec::new();
    for entry in &listing.entries {
        let unchanged = old_entries
            .get(entry.path.as_str())
            .is_some_and(|previous| same_entry(previous, entry));
        let resolved = previous_resolved.contains(entry.path.as_str());
        if !unchanged || !resolved {
            missing.push(DecorationEntry {
                path: entry.path.clone(),
                name: entry.name.clone(),
                is_dir: entry.is_dir,
                extension: entry.extension.clone(),
                mime: entry.mime.clone(),
                allowed_ops: entry.allowed_ops.clone(),
                tags: entry
                    .tags
                    .iter()
                    .map(|tag| DecorationTag {
                        name: tag.name.clone(),
                        color: tag.color,
                        hex: None,
                    })
                    .collect(),
                sync: entry.sync.clone(),
                has_cache: entry.has_cache,
                pinned: entry.pinned,
                uploaded: entry.uploaded,
                link_target: entry.link_target.clone(),
            });
        }
    }
    let resolved = if missing.is_empty() {
        Vec::new()
    } else {
        query_with_plugins(path, missing, true)
    };
    decorations.extend(resolved.iter().cloned());
    if original_identity == key(path) && enabled(path) {
        disk::store(
            &identity(path),
            &Snapshot {
                listing: listing.clone(),
                decorations: decorations.clone(),
                resolved_paths: listing
                    .entries
                    .iter()
                    .map(|entry| entry.path.clone())
                    .collect(),
                stamp: current_stamp,
            },
        );
    }
    resolved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incremental_listing_reuses_cached_icons_for_unchanged_entries() {
        let directory = "applications://folder_decoration-cache-incremental";
        let existing = crate::core::files::Entry {
            path: "applications://app_decoration-cache-existing".into(),
            name: "Existing app".into(),
            ..Default::default()
        };
        let listing = Listing {
            allowed_ops: vec!["list".into()],
            virtual_volume: true,
            path: directory.into(),
            entries: vec![existing.clone()],
            warnings: vec![],
            default_view: None,
        };
        super::super::directory_cache::store(directory, &listing);
        store(
            directory,
            &listing,
            &[Decoration {
                path: existing.path.clone(),
                icon: Some("data:image/png;base64,cached-logo".into()),
                ..Default::default()
            }],
        );

        super::super::directory_cache::add_entry(
            directory,
            crate::core::files::Entry {
                path: "applications://app_decoration-cache-new".into(),
                name: "New app".into(),
                ..Default::default()
            },
        );
        let updated = super::super::directory_cache::read(directory, true, &[])
            .unwrap()
            .unwrap();
        let (decorations, complete) = cached(directory, &updated);

        assert!(!complete);
        assert_eq!(decorations.len(), 1);
        assert_eq!(decorations[0].path, existing.path);
        assert_eq!(
            decorations[0].icon.as_deref(),
            Some("data:image/png;base64,cached-logo")
        );
        super::super::directory_cache::drop_subtree(directory);
    }
}
