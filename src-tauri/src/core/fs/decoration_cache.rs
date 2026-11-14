//! Presentation snapshots are created exclusively by the directory view boundary.
#[cfg(any(feature = "desktop", test))]
use super::directory_cache::{enabled, key};
use crate::infrastructure::cache::disk;
#[cfg(any(feature = "desktop", test))]
use crate::{core::files::Listing, plugins::providers::decorations::Decoration};
#[cfg(any(feature = "desktop", test))]
mod snapshot;
#[cfg(any(feature = "desktop", test))]
use snapshot::{Snapshot, reuse_from_snapshot};
#[cfg(feature = "desktop")]
use snapshot::{listing_index, same_entry};
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
fn identity(path: &str) -> String {
    format!("decoration:{}", key(path))
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
        DecorationEntry, DecorationTag, query_with_plugins,
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
        let directory = "applications://Decoration%20Cache";
        let existing = crate::core::files::Entry {
            path: "applications://Decoration%20Cache/Existing%20App".into(),
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
                path: "applications://Decoration%20Cache/New%20App".into(),
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
