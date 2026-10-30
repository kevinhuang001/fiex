//! Short-lived, directory-scoped mutations layered over complete VFS snapshots.
//! A provider may return an older list immediately after a successful write.
//! Keep the committed local change visible until a fresh list confirms it or
//! the bounded reconciliation window expires.
use crate::core::files::{Entry, Listing};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, OnceLock,
    },
    time::{Duration, Instant},
};

const RECONCILE_WINDOW: Duration = Duration::from_secs(60);
const MAX_PENDING_PER_DIRECTORY: usize = 512;
static CLOCK: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
enum Kind {
    Upsert(Entry),
    Remove,
}
struct Pending {
    path: String,
    change: Kind,
    at: Instant,
}
#[derive(Default)]
struct Directory {
    generation: u64,
    pending: Vec<Pending>,
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
    directory.pending.retain(|pending| pending.path != path);
    directory.pending.push(Pending {
        path: path.into(),
        change,
        at: Instant::now(),
    });
    if directory.pending.len() > MAX_PENDING_PER_DIRECTORY {
        directory.pending.remove(0);
    }
}
pub(super) fn upsert(parent: &str, entry: Entry) {
    record(parent, &entry.path.clone(), Kind::Upsert(entry));
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
fn apply(listing: &mut Listing, pending: &Pending) {
    match &pending.change {
        Kind::Upsert(entry) => {
            if let Some(current) = listing
                .entries
                .iter_mut()
                .find(|item| item.path == entry.path)
            {
                *current = entry.clone();
            } else {
                listing.entries.push(entry.clone());
            }
        }
        Kind::Remove => listing.entries.retain(|entry| entry.path != pending.path),
    }
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
        for pending in &directory.pending {
            if pending.at.elapsed() < RECONCILE_WINDOW {
                apply(listing, pending);
            }
        }
        if !directory.pending.is_empty() {
            order(listing);
        }
    }
}
/// Only a *live* provider listing can acknowledge a mutation. Old provider
/// data is merged with the overlay; changed rows are replaced in-place.
pub(super) fn reconcile(path: &str, listing: &mut Listing) {
    let mut directories = registry().lock().unwrap_or_else(|error| error.into_inner());
    let Some(directory) = directories.get_mut(&identity(path)) else {
        return;
    };
    directory.pending.retain(|pending| {
        if pending.at.elapsed() >= RECONCILE_WINDOW {
            return false;
        }
        let current = listing
            .entries
            .iter()
            .find(|entry| entry.path == pending.path);
        let confirmed = match &pending.change {
            Kind::Remove => current.is_none(),
            Kind::Upsert(expected) => current.is_some_and(|entry| {
                entry.name == expected.name
                    && entry.is_dir == expected.is_dir
                    && entry.size == expected.size
                    && entry.modified == expected.modified
            }),
        };
        if confirmed {
            return false;
        }
        apply(listing, pending);
        true
    });
    order(listing);
}
#[cfg(test)]
mod tests {
    use super::*;
    fn listing(path: &str, entries: Vec<Entry>) -> Listing {
        Listing {
            path: path.into(),
            entries,
            read_only: false,
            can_create: true,
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
        let dir = "applications://user/overlay-test-create";
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
        let dir = "applications://user/overlay-test-remove";
        let node = format!("{dir}/old");
        remove(dir, &node);
        let mut old = listing(dir, vec![entry(&node)]);
        reconcile(dir, &mut old);
        assert!(old.entries.is_empty());
        discard(dir);
    }
    #[test]
    fn mutations_are_directory_scoped() {
        let first = "applications://user/overlay-test-a";
        let second = "applications://user/overlay-test-b";
        let before = generation(second);
        upsert(first, entry(&format!("{first}/new")));
        assert_eq!(generation(second), before);
        discard(first);
    }
}
