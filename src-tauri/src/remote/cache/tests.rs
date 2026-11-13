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
