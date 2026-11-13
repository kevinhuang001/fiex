//! Short-lived, directory-scoped mutations layered over complete VFS snapshots.
//! A provider may return an older list immediately after a successful write.
//! Keep the committed local change visible until a fresh list confirms it or
//! the bounded reconciliation window expires.
use crate::core::files::{Entry, Listing};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

const RECONCILE_WINDOW: Duration = Duration::from_secs(60);
const MAX_PENDING_PER_DIRECTORY: usize = 512;
static CLOCK: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
enum Kind {
    Upsert(Box<Entry>),
    Remove,
}
#[derive(Clone)]
struct Pending {
    change: Kind,
    at: Instant,
}
#[derive(Default)]
struct Directory {
    generation: u64,
    pending: HashMap<String, Pending>,
}
fn registry() -> &'static Mutex<HashMap<String, Directory>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, Directory>>> = OnceLock::new();
    REGISTRY.get_or_init(Default::default)
}
fn identity(path: &str) -> String {
    super::directory_cache::key(path)
}
pub(super) fn generation(path: &str) -> u64 {
    registry()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get(&identity(path))
        .map_or(0, |directory| directory.generation)
}
fn record(parent: &str, path: &str, change: Kind) {
    let mut directories = registry().lock().unwrap_or_else(|error| error.into_inner());
    let directory = directories.entry(identity(parent)).or_default();
    directory.generation = CLOCK.fetch_add(1, Ordering::AcqRel);
    let _ = directory.pending.insert(
        path.into(),
        Pending {
            change,
            at: Instant::now(),
        },
    );
    if directory.pending.len() > MAX_PENDING_PER_DIRECTORY
        && let Some(oldest) = directory
            .pending
            .iter()
            .min_by_key(|(_, pending)| pending.at)
            .map(|(path, _)| path.clone())
    {
        let _ = directory.pending.remove(&oldest);
    }
}
pub(super) fn upsert(parent: &str, entry: Entry) {
    record(parent, &entry.path.clone(), Kind::Upsert(Box::new(entry)));
}
pub(super) fn remove(parent: &str, path: &str) {
    record(parent, path, Kind::Remove);
}
/// Used if an operation cannot return authoritative new metadata.
pub(super) fn discard(parent: &str) {
    let mut directories = registry().lock().unwrap_or_else(|error| error.into_inner());
    let directory = directories.entry(identity(parent)).or_default();
    directory.generation = CLOCK.fetch_add(1, Ordering::AcqRel);
    directory.pending.clear();
}
fn apply(listing: &mut Listing, pending: &HashMap<String, Pending>) {
    if pending.is_empty() {
        return;
    }
    let mut positions: HashMap<String, usize> = listing
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| (entry.path.clone(), index))
        .collect();
    let mut removed = HashSet::new();
    for (path, change) in pending {
        match &change.change {
            Kind::Upsert(entry) => {
                if let Some(index) = positions.get(path).copied() {
                    listing.entries[index] = entry.as_ref().clone();
                } else {
                    positions.insert(path.clone(), listing.entries.len());
                    listing.entries.push(entry.as_ref().clone());
                }
                removed.remove(path.as_str());
            }
            Kind::Remove => {
                removed.insert(path.as_str());
            }
        }
    }
    if !removed.is_empty() {
        listing
            .entries
            .retain(|entry| !removed.contains(entry.path.as_str()));
    }
    order(listing);
}
fn order(listing: &mut Listing) {
    listing
        .entries
        .sort_by_cached_key(|entry| (!entry.is_dir, entry.name.to_lowercase()));
}
/// Cache paint never acknowledges a mutation: cached data may already include
/// an optimistic entry and is not proof that the provider has caught up.
pub(super) fn paint(path: &str, listing: &mut Listing) {
    let directories = registry().lock().unwrap_or_else(|error| error.into_inner());
    if let Some(directory) = directories.get(&identity(path)) {
        let active: HashMap<_, _> = directory
            .pending
            .iter()
            .filter(|(_, pending)| pending.at.elapsed() < RECONCILE_WINDOW)
            .map(|(path, pending)| (path.clone(), pending.clone()))
            .collect();
        apply(listing, &active);
    }
}
/// Only a *live* provider listing can acknowledge a mutation. Old provider
/// data is merged with the overlay; changed rows are replaced in-place.
pub(super) fn reconcile_at_revision(path: &str, listing: &mut Listing, revision: u64) -> bool {
    let mut directories = registry().lock().unwrap_or_else(|error| error.into_inner());
    let Some(directory) = directories.get_mut(&identity(path)) else {
        return revision == 0;
    };
    if directory.generation != revision {
        return false;
    }
    let current: HashMap<_, _> = listing
        .entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect();
    directory.pending.retain(|path, pending| {
        if pending.at.elapsed() >= RECONCILE_WINDOW {
            return false;
        }
        let entry = current.get(path.as_str()).copied();
        let confirmed = match &pending.change {
            Kind::Remove => entry.is_none(),
            Kind::Upsert(expected) => entry.is_some_and(|entry| {
                entry.name == expected.name
                    && entry.is_dir == expected.is_dir
                    && entry.size == expected.size
                    && entry.modified == expected.modified
            }),
        };
        !confirmed
    });
    apply(listing, &directory.pending);
    true
}
#[cfg(test)]
mod tests {
    use super::*;
    fn reconcile(path: &str, listing: &mut Listing) {
        let revision = generation(path);
        let _ = reconcile_at_revision(path, listing, revision);
    }

    fn listing(path: &str, entries: Vec<Entry>) -> Listing {
        Listing {
            path: path.into(),
            entries,
            allowed_ops: vec![
                "list".into(),
                "stat".into(),
                "search".into(),
                "write".into(),
                "mkdir".into(),
            ],
            virtual_volume: true,
            warnings: vec![],
            default_view: None,
        }
    }
    fn entry(path: &str) -> Entry {
        Entry {
            path: path.into(),
            name: path.rsplit('/').next().unwrap().into(),
            ..Default::default()
        }
    }
    #[test]
    fn stale_live_listing_does_not_erase_committed_create() {
        let dir = "applications://app/overlay-test-create";
        let node = format!("{dir}/new");
        upsert(dir, entry(&node));
        let mut old = listing(dir, vec![]);
        reconcile(dir, &mut old);
        assert_eq!(old.entries.len(), 1);
        let mut fresh = listing(dir, vec![entry(&node)]);
        reconcile(dir, &mut fresh);
        assert_eq!(fresh.entries.len(), 1);
        discard(dir);
    }
    #[test]
    fn removed_entry_stays_hidden_until_confirmed() {
        let dir = "applications://app/overlay-test-remove";
        let node = format!("{dir}/old");
        remove(dir, &node);
        let mut old = listing(dir, vec![entry(&node)]);
        reconcile(dir, &mut old);
        assert!(old.entries.is_empty());
        discard(dir);
    }
    #[test]
    fn mutations_are_directory_scoped() {
        let first = "applications://app/overlay-test-a";
        let second = "applications://app/overlay-test-b";
        let before = generation(second);
        upsert(first, entry(&format!("{first}/new")));
        assert_eq!(generation(second), before);
        discard(first);
    }

    #[test]
    fn indexed_reconciliation_applies_distinct_changes_once() {
        let dir = "applications://folder/overlay-indexed-reconcile";
        let updated_path = format!("{dir}/updated");
        let removed_path = format!("{dir}/removed");
        let added_path = format!("{dir}/added");
        let mut updated = entry(&updated_path);
        updated.size = 42;
        upsert(dir, updated);
        remove(dir, &removed_path);
        upsert(dir, entry(&added_path));

        let mut stale = listing(dir, vec![entry(&updated_path), entry(&removed_path)]);
        reconcile(dir, &mut stale);

        assert_eq!(stale.entries.len(), 2);
        assert!(stale.entries.iter().any(|item| item.path == added_path));
        assert!(
            stale
                .entries
                .iter()
                .any(|item| { item.path == updated_path && item.size == 42 })
        );
        assert!(!stale.entries.iter().any(|item| item.path == removed_path));
        discard(dir);
    }
}
