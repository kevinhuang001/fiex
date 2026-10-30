//! Presentation snapshots are created exclusively by the directory view boundary.
#[cfg(any(feature = "desktop", test))]
use super::directory_cache::{enabled, key};
use crate::infrastructure::cache::disk;
#[cfg(any(feature = "desktop", test))]
use crate::{core::files::Listing, plugins::resolvers::decorations::Decoration};
#[cfg(any(feature = "desktop", test))]
use serde::{Deserialize, Serialize};
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
#[cfg(test)]
pub(crate) fn read(path: &str, listing: &Listing) -> Option<Vec<Decoration>> {
    if !enabled(path) {
        return None;
    }
    let data: Snapshot = disk::load(&identity(path))?;
    let matches = data.stamp == stamp()
        && listing.entries.iter().all(|entry| {
            data.listing.entries.iter().any(|old| {
                old.path == entry.path
                    && serde_json::to_value(old).ok() == serde_json::to_value(entry).ok()
            })
        });
    matches.then(|| {
        data.decorations
            .into_iter()
            .filter(|d| listing.entries.iter().any(|e| e.path == d.path))
            .collect()
    })
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
    use crate::plugins::resolvers::decorations::{query_with_plugins, DecorationEntry};
    let original_identity = key(path);
    let previous: Option<Snapshot> = enabled(path).then(|| disk::load(&identity(path))).flatten();
    let previous = previous.filter(|snapshot| snapshot.stamp == stamp());
    let mut decorations = Vec::new();
    let mut missing: Vec<DecorationEntry> = Vec::new();
    for entry in &listing.entries {
        let unchanged = previous.as_ref().is_some_and(|snapshot| {
            snapshot.listing.entries.iter().any(|old| {
                old.path == entry.path
                    && serde_json::to_value(old).ok() == serde_json::to_value(entry).ok()
            })
        });
        if unchanged {
            if let Some(snapshot) = &previous {
                decorations.extend(
                    snapshot
                        .decorations
                        .iter()
                        .filter(|d| d.path == entry.path)
                        .cloned(),
                );
            }
        } else if let Ok(value) = serde_json::to_value(entry) {
            if let Ok(value) = serde_json::from_value(value) {
                missing.push(value);
            }
        }
    }
    if !missing.is_empty() {
        decorations.extend(query_with_plugins(path, missing, true));
    }
    if original_identity == key(path) {
        store(path, listing, &decorations);
    }
    decorations
}
