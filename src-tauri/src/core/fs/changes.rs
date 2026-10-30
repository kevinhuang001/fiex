//! Successful mutations are reconciled per directory, never by evicting all
//! parent snapshots or advancing a single global list revision.
use super::{binding, directory_cache};

pub(crate) fn revision(path: &str) -> u64 {
    super::mutation_overlay::generation(path)
}
/// Commit a created or modified entry. If its authoritative stat fails, fall
/// back to invalidation rather than inventing attributes or caching a failure.
pub(crate) fn committed(path: &str) {
    if !directory_cache::enabled(path) {
        return;
    }
    let Ok(parent) = binding::parent_path(path) else {
        return;
    };
    let updated = binding::trusted(path, "core:mutation")
        .and_then(|broker| broker.desktop_entry().map_err(String::from));
    match updated {
        Ok(entry) => directory_cache::add_entry(&parent, entry),
        Err(_) => directory_cache::invalidate_directory(&parent),
    }
}
/// Use the entry returned by mkdir itself: a second remote stat is unnecessary
/// and may temporarily fail on eventually consistent providers.
pub(crate) fn committed_result(path: &str, value: &serde_json::Value) {
    if !directory_cache::enabled(path) {
        return;
    }
    let Ok(parent) = binding::parent_path(path) else {
        return;
    };
    match super::presentation::entry(value.clone()) {
        Ok(mut entry) => {
            entry.path = path.into();
            directory_cache::add_entry(&parent, entry);
        }
        Err(_) => committed(path),
    }
}
pub(crate) fn removed(path: &str) {
    if !directory_cache::enabled(path) {
        return;
    }
    if let Ok(parent) = binding::parent_path(path) {
        directory_cache::remove_entry(&parent, path);
    }
}
pub(crate) fn renamed(old: &str, new: &str) {
    if !directory_cache::enabled(old) && !directory_cache::enabled(new) {
        return;
    }
    let old_parent = binding::parent_path(old);
    let new_parent = binding::parent_path(new);
    let updated = binding::trusted(new, "core:mutation")
        .and_then(|broker| broker.desktop_entry().map_err(String::from));
    match (old_parent, new_parent, updated) {
        (Ok(source), Ok(target), Ok(entry)) if source == target => {
            directory_cache::rename_entry(&source, old, entry);
        }
        (Ok(source), Ok(target), Ok(entry)) => {
            directory_cache::remove_entry(&source, old);
            directory_cache::add_entry(&target, entry);
        }
        (source, target, _) => {
            if let Ok(source) = source {
                directory_cache::invalidate_directory(&source);
            }
            if let Ok(target) = target {
                directory_cache::invalidate_directory(&target);
            }
        }
    }
}
