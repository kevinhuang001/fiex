use super::*;
fn listing(path: &str, entries: Vec<Entry>) -> Listing {
    Listing {
        path: path.into(),
        entries,
        allowed_ops: vec!["list".into(), "stat".into(), "mkdir".into()],
        virtual_volume: true,
        warnings: vec![],
        default_view: None,
    }
}
fn item(path: &str, name: &str, directory: bool) -> Entry {
    Entry {
        path: path.into(),
        name: name.into(),
        is_dir: directory,
        ..Default::default()
    }
}
#[test]
fn local_listing_never_builds_directory_snapshots() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().to_str().unwrap();
    std::fs::write(root.path().join("needle.txt"), b"x").unwrap();
    let broker = super::super::trusted(path, "core:test").unwrap();
    broker.list().unwrap();
    broker.desktop_listing(true, &[]).unwrap();
    store(path, &listing(path, vec![]));
    assert!(read(path, true, &[]).unwrap().is_none());
    assert!(!enabled(path));
}
#[test]
fn empty_vfs_snapshot_is_complete_and_does_not_create_decorations() {
    let path = "applications://folder/cache-test-empty";
    store(path, &listing(path, vec![]));
    let cached = read(path, true, &[]).unwrap().unwrap();
    assert!(cached.entries.is_empty());
    assert!(super::super::decoration_cache::read(path, &cached).is_none());
    drop_subtree(path);
}

#[test]
fn cached_entry_uses_the_authorized_directory_snapshot() {
    let directory = "applications://folder/cache-test-entry";
    let child = "applications://app/cache-test-entry-note".to_string();
    let mut cached = item(&child, "notes.txt", false);
    cached.allowed_ops = vec!["stat".into(), "read".into()];
    store(directory, &listing(directory, vec![cached.clone()]));

    assert_eq!(
        cached_entry(directory, &child).unwrap().unwrap().path,
        child
    );
    assert!(cached_entry(directory, "applications://app/other-note")
        .unwrap()
        .is_none());
    drop_subtree(directory);
}
#[test]
fn cached_tree_respects_parent_edges_hidden_filters_and_cancellation() {
    let path = "applications://folder/cache-test-tree";
    let child = "applications://folder/cache-test-tree-child".to_string();
    let stale = "applications://folder/cache-test-tree-deleted".to_string();
    store(path, &listing(path, vec![item(&child, ".hidden", true)]));
    store(
        &child,
        &listing(
            &child,
            vec![item(
                "applications://app/cache-test-tree-needle",
                "Needle",
                false,
            )],
        ),
    );
    store(
        &stale,
        &listing(
            &stale,
            vec![item(
                "applications://app/cache-test-tree-stale-needle",
                "needle",
                false,
            )],
        ),
    );
    let mut found = Vec::new();
    search(path, "NEEDLE", false, &[], &|| false, &mut |entry| {
        found.push(entry)
    })
    .unwrap();
    assert!(found.is_empty());
    search(path, "NEEDLE", true, &[], &|| false, &mut |entry| {
        found.push(entry)
    })
    .unwrap();
    assert_eq!(found.len(), 1); // Deleted parent never exposes its stale snapshot.
    found.clear();
    search(path, "needle", true, &[], &|| true, &mut |entry| {
        found.push(entry)
    })
    .unwrap();
    assert!(found.is_empty());
    drop_subtree(path);
}

#[test]
fn deleted_child_and_descendants_do_not_leave_stale_directory_snapshots() {
    let parent = "applications://folder/cache-test-delete".to_string();
    let child = "applications://folder/cache-test-delete-child".to_string();
    store(
        &parent,
        &listing(&parent, vec![item(&child, "child", true)]),
    );
    store(
        &child,
        &listing(
            &child,
            vec![item(
                "applications://app/cache-test-delete-needle",
                "needle",
                false,
            )],
        ),
    );
    drop_subtree(&child);
    assert!(read(&child, true, &[]).unwrap().is_none());
    assert!(read(&parent, true, &[]).unwrap().is_some());
    drop_subtree(&parent);
    assert!(read(&parent, true, &[]).unwrap().is_none());
}

#[test]
fn invalidation_uses_parent_edges_and_does_not_remove_unrelated_siblings() {
    let parent = "applications://folder/cache-test-siblings".to_string();
    let first = "applications://folder/cache-test-sibling-first".to_string();
    let second = "applications://folder/cache-test-sibling-second".to_string();
    store(
        &parent,
        &listing(
            &parent,
            vec![item(&first, "first", true), item(&second, "second", true)],
        ),
    );
    store(&first, &listing(&first, vec![]));
    store(&second, &listing(&second, vec![]));
    drop_subtree(&first);
    assert!(read(&first, true, &[]).unwrap().is_none());
    assert!(read(&second, true, &[]).unwrap().is_some());
    drop_subtree(&parent);
}

#[test]
fn presentation_snapshot_requires_matching_entry_metadata() {
    let path = "applications://folder/cache-test-presentation";
    let value = listing(
        path,
        vec![item(
            "applications://app/cache-test-presentation-app",
            "app",
            false,
        )],
    );
    super::super::decoration_cache::store(path, &value, &[]);
    assert!(super::super::decoration_cache::read(path, &value).is_some());
    let mut changed = value.clone();
    changed.entries[0].modified = 42;
    assert!(super::super::decoration_cache::read(path, &changed).is_none());
    store(path, &value);
    drop_subtree(path);
}
