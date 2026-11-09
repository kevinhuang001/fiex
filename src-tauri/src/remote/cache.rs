//! Cache and pin state helpers shared by listings, reconciliation and menus.

use serde::Serialize;

use super::{
    config::{local_for, navigation_path},
    store::StoredConnection,
    sync::{RemoteMeta, dirty, in_tree},
};

/// A cache or pin transition detected by background reconciliation. The
/// desktop shell forwards these to the activity centre.
#[derive(Clone, Serialize)]
pub struct CacheChange {
    pub path: String,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

pub(crate) fn cache_change(conn: &StoredConnection, rel: &str, state: &str) -> CacheChange {
    CacheChange {
        path: navigation_path(conn, rel).unwrap_or_else(|_| rel.to_string()),
        state: state.into(),
        detail: None,
    }
}

/// A failed sync step; the activity centre shows the path and the reason.
pub(crate) fn cache_failure(conn: &StoredConnection, rel: &str, error: &str) -> CacheChange {
    CacheChange {
        path: navigation_path(conn, rel).unwrap_or_else(|_| rel.to_string()),
        state: "failed".into(),
        detail: Some(error.to_string()),
    }
}

/// The single cache projection used by listings, stat and menu snapshots.
/// `cached` means the complete node is available offline; `has_cache` also
/// includes partial directory materialization so it can be evicted safely.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CacheState {
    pub cached: bool,
    pub has_cache: bool,
    pub partial: bool,
    pub pinned: bool,
    pub uploaded: bool,
    pub dirty: bool,
    pub conflict: bool,
}

/// Project cache state for a group of listing entries in one pass. Directory
/// descendants are indexed once, avoiding a full metadata scan per row.
pub(crate) fn states_for(
    conn: &StoredConnection,
    meta: &RemoteMeta,
    nodes: &[(&str, bool)],
) -> std::collections::HashMap<String, CacheState> {
    #[derive(Default)]
    struct Descendants {
        cached_bytes: bool,
        pinned: bool,
        conflict: bool,
    }

    let mut descendants = std::collections::HashMap::<String, Descendants>::new();
    for (key, file) in &meta.files {
        let cached_bytes = !file.is_dir && file.downloaded && local_for(conn, key).is_file();
        let mut ancestor = Some(key.as_str());
        while let Some(path) = ancestor {
            let state = descendants.entry(path.to_owned()).or_default();
            state.cached_bytes |= cached_bytes;
            state.pinned |= file.pinned;
            state.conflict |= file.conflict;
            ancestor = path.rsplit_once('/').map(|(parent, _)| parent);
        }
        let root = descendants.entry(String::new()).or_default();
        root.cached_bytes |= cached_bytes;
        root.pinned |= file.pinned;
        root.conflict |= file.conflict;
    }

    let complete_directories = cached_dirs(conn, meta);
    nodes
        .iter()
        .map(|(rel, is_dir)| {
            let file = meta.files.get(*rel);
            let aggregate = descendants.get(*rel);
            let cached = if *is_dir {
                complete_directories.contains(*rel)
            } else {
                file.is_some_and(|entry| entry.downloaded && local_for(conn, rel).is_file())
            };
            let has_cache = if *is_dir {
                cached || aggregate.is_some_and(|state| state.cached_bytes)
            } else {
                cached
            };
            let dirty = !is_dir && file.is_some_and(|entry| dirty(conn, rel, entry));
            let conflict = aggregate.is_some_and(|state| state.conflict);
            let pinned = aggregate.is_some_and(|state| state.pinned);
            let uploaded = !is_dir
                && file.is_some_and(|entry| entry.uploaded && !entry.conflict)
                && !dirty
                && (has_cache || pinned);
            (
                (*rel).to_owned(),
                CacheState {
                    cached,
                    has_cache,
                    partial: has_cache && !cached,
                    pinned,
                    uploaded,
                    dirty,
                    conflict,
                },
            )
        })
        .collect()
}

pub(crate) fn state(
    conn: &StoredConnection,
    meta: &RemoteMeta,
    rel: &str,
    is_dir: bool,
) -> CacheState {
    let cached_dirs = cached_dirs(conn, meta);
    state_with_cached_dirs(conn, meta, rel, is_dir, &cached_dirs)
}

pub(crate) fn state_with_cached_dirs(
    conn: &StoredConnection,
    meta: &RemoteMeta,
    rel: &str,
    is_dir: bool,
    cached_dirs: &std::collections::HashSet<String>,
) -> CacheState {
    let file = meta.files.get(rel);
    let cached = if is_dir {
        cached_dirs.contains(rel)
    } else {
        file.is_some_and(|entry| entry.downloaded && local_for(conn, rel).is_file())
    };
    let has_cached_bytes = meta.files.iter().any(|(key, entry)| {
        !entry.is_dir && entry.downloaded && in_tree(key, rel) && local_for(conn, key).is_file()
    });
    let has_cache = if is_dir {
        cached || has_cached_bytes
    } else {
        cached
    };
    let dirty = !is_dir && file.is_some_and(|entry| dirty(conn, rel, entry));
    let conflict = meta
        .files
        .iter()
        .any(|(key, entry)| in_tree(key, rel) && entry.conflict);
    let pinned = meta
        .files
        .iter()
        .any(|(key, entry)| in_tree(key, rel) && entry.pinned);
    let uploaded = !is_dir
        && file.is_some_and(|entry| entry.uploaded && !entry.conflict)
        && !dirty
        && (has_cache || pinned);
    CacheState {
        cached,
        has_cache,
        partial: has_cache && !cached,
        pinned,
        uploaded,
        dirty,
        conflict,
    }
}

/// A directory is offline-complete only after its full paginated listing was
/// visited, its local directory exists, and every recorded child is complete.
/// Empty directories are complete when their listing is complete.
pub(crate) fn cached_dirs(
    conn: &StoredConnection,
    meta: &RemoteMeta,
) -> std::collections::HashSet<String> {
    cached_dirs_with(
        meta,
        |rel| local_for(conn, rel).is_dir(),
        |rel| local_for(conn, rel).is_file(),
    )
}

fn cached_dirs_with(
    meta: &RemoteMeta,
    is_local_dir: impl Fn(&str) -> bool,
    is_local_file: impl Fn(&str) -> bool,
) -> std::collections::HashSet<String> {
    use std::collections::{HashMap, HashSet};
    fn complete<F, G>(
        meta: &RemoteMeta,
        rel: &str,
        children: &HashMap<String, Vec<String>>,
        visiting: &mut HashSet<String>,
        completed: &mut HashMap<String, bool>,
        is_local_dir: &F,
        is_local_file: &G,
    ) -> bool
    where
        F: Fn(&str) -> bool,
        G: Fn(&str) -> bool,
    {
        if let Some(value) = completed.get(rel) {
            return *value;
        }
        if !visiting.insert(rel.to_owned()) || !meta.listed.contains(rel) || !is_local_dir(rel) {
            return false;
        }
        let result = children.get(rel).into_iter().flatten().all(|child| {
            let Some(entry) = meta.files.get(child) else {
                return false;
            };
            if entry.is_dir {
                complete(
                    meta,
                    child,
                    children,
                    visiting,
                    completed,
                    is_local_dir,
                    is_local_file,
                )
            } else {
                entry.downloaded && is_local_file(child)
            }
        });
        visiting.remove(rel);
        completed.insert(rel.to_owned(), result);
        result
    }

    let mut children: HashMap<String, Vec<String>> = HashMap::new();
    for key in meta.files.keys() {
        let parent = key.rsplit_once('/').map(|(parent, _)| parent).unwrap_or("");
        children.entry(parent.into()).or_default().push(key.clone());
    }
    for (parent, listed) in &meta.listed_children {
        children.insert(parent.clone(), listed.clone());
    }
    let mut completed = HashMap::new();
    let mut dirs = HashSet::new();
    let mut candidates = meta.listed.iter().cloned().collect::<Vec<_>>();
    if meta.listed.contains("") {
        candidates.push(String::new());
    }
    candidates.sort();
    candidates.dedup();
    for rel in candidates {
        if complete(
            meta,
            &rel,
            &children,
            &mut HashSet::new(),
            &mut completed,
            &is_local_dir,
            &is_local_file,
        ) {
            dirs.insert(rel);
        }
    }
    dirs
}

/// Cache entries with bytes on disk, used to diff one reconciliation round.
pub(crate) fn downloaded_keys(meta: &RemoteMeta) -> std::collections::HashSet<String> {
    meta.files
        .iter()
        .filter(|(_, file)| file.downloaded)
        .map(|(key, _)| key.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote::sync::RemoteMetaFile;
    use std::collections::HashSet;

    fn cached_dirs_for(
        meta: &RemoteMeta,
        directories: &[&str],
        files: &[&str],
    ) -> std::collections::HashSet<String> {
        let directories = directories.iter().copied().collect::<HashSet<_>>();
        let files = files.iter().copied().collect::<HashSet<_>>();
        cached_dirs_with(
            meta,
            |rel| directories.contains(rel),
            |rel| files.contains(rel),
        )
    }

    fn cached(is_dir: bool) -> RemoteMetaFile {
        RemoteMetaFile {
            downloaded: true,
            is_dir,
            ..Default::default()
        }
    }

    fn test_connection() -> StoredConnection {
        StoredConnection {
            id: format!(
                "cache-state-test-{}",
                crate::plugins::mounts::new_id().unwrap()
            ),
            mount_id: String::new(),
            cache_revision: String::new(),
            name: String::new(),
            kind: String::new(),
            plugin_id: String::new(),
            settings: serde_json::Map::new(),
            secret_keys: Vec::new(),
            saved: true,
            autostart: false,
        }
    }

    #[test]
    fn batch_projection_aggregates_directory_pin_and_conflict_state() {
        let conn = test_connection();
        let mut meta = RemoteMeta::default();
        meta.files.insert(
            "folder".into(),
            RemoteMetaFile {
                is_dir: true,
                pinned: true,
                ..Default::default()
            },
        );
        meta.files.insert(
            "folder/file.txt".into(),
            RemoteMetaFile {
                conflict: true,
                pinned: true,
                ..Default::default()
            },
        );
        let states = states_for(
            &conn,
            &meta,
            &[
                ("folder", true),
                ("folder/file.txt", false),
                ("uncached.txt", false),
            ],
        );
        assert!(states["folder"].pinned && states["folder"].conflict);
        assert!(states["folder/file.txt"].pinned && states["folder/file.txt"].conflict);
        assert!(!states["uncached.txt"].has_cache && !states["uncached.txt"].pinned);
    }

    #[test]
    fn cached_dirs_cover_every_ancestor() {
        let mut meta = RemoteMeta::default();
        meta.files.insert(
            "a".into(),
            RemoteMetaFile {
                is_dir: true,
                ..Default::default()
            },
        );
        meta.files.insert(
            "a/b".into(),
            RemoteMetaFile {
                is_dir: true,
                ..Default::default()
            },
        );
        meta.files.insert("a/b/c.txt".into(), cached(false));
        meta.listed.extend(["a".into(), "a/b".into()]);
        let dirs = cached_dirs_for(&meta, &["a", "a/b"], &["a/b/c.txt"]);
        assert!(dirs.contains("a"));
        assert!(dirs.contains("a/b"));
        assert!(!dirs.contains(""));
    }

    #[test]
    fn a_partially_cached_folder_is_not_marked_cached() {
        let mut meta = RemoteMeta::default();
        meta.files.insert(
            "a".into(),
            RemoteMetaFile {
                is_dir: true,
                ..Default::default()
            },
        );
        meta.files.insert("a/one.txt".into(), cached(false));
        meta.files.insert(
            "a/two.txt".into(),
            RemoteMetaFile {
                is_dir: false,
                downloaded: false,
                ..Default::default()
            },
        );
        meta.listed.insert("a".into());
        assert!(!cached_dirs_for(&meta, &["a"], &["a/one.txt"]).contains("a"));
        meta.files.get_mut("a/two.txt").unwrap().downloaded = true;
        assert!(cached_dirs_for(&meta, &["a"], &["a/one.txt", "a/two.txt"]).contains("a"));
    }

    #[test]
    fn complete_empty_directories_are_cached() {
        let mut meta = RemoteMeta::default();
        meta.files.insert(
            "empty".into(),
            RemoteMetaFile {
                is_dir: true,
                ..Default::default()
            },
        );
        meta.listed.insert("empty".into());
        assert!(cached_dirs_for(&meta, &["empty"], &[]).contains("empty"));
    }

    #[test]
    fn downloads_are_not_pins() {
        let meta: RemoteMeta = serde_json::from_str(
            r#"{"files":{"a":{"size":1,"modified":0,"isDir":false,"downloaded":true,"nodeId":"a","revision":"r1","baseline":"","localStamp":"","pinned":false,"excluded":false,"conflict":false,"uploaded":false}},"listed":[],"listed_children":{},"errors":{}}"#,
        )
        .unwrap();
        assert!(meta.files["a"].downloaded);
        assert!(!meta.files["a"].pinned);
        assert!(cached_dirs_for(&meta, &[], &["a"]).is_empty());
    }
}
