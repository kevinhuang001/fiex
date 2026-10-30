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
    let path = "applications://user/cache-test-empty";
    store(path, &listing(path, vec![]));
    let cached = read(path, true, &[]).unwrap().unwrap();
    assert!(cached.entries.is_empty());
    assert!(super::super::decoration_cache::read(path, &cached).is_none());
    drop_subtree(path);
}
#[test]
fn cached_tree_respects_parent_edges_hidden_filters_and_cancellation() {
    let path = "applications://user/cache-test-tree";
    let child = format!("{path}/.hidden");
    let stale = format!("{path}/deleted");
    store(path, &listing(path, vec![item(&child, ".hidden", true)]));
    store(
        &child,
        &listing(
            &child,
            vec![item(&format!("{child}/Needle"), "Needle", false)],
        ),
    );
    store(
        &stale,
        &listing(
            &stale,
            vec![item(&format!("{stale}/needle"), "needle", false)],
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
    let root = tempfile::tempdir().unwrap();
    let parent = root.path().join("visited");
    let child = parent.join("child");
    std::fs::create_dir_all(&child).unwrap();
    let parent = "applications://user/cache-test-delete".to_string();
    let child = format!("{parent}/child");
    store(
        &parent,
        &listing(&parent, vec![item(&child, "child", true)]),
    );
    store(
        &child,
        &listing(
            &child,
            vec![item(&format!("{child}/needle"), "needle", false)],
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
    let parent = "applications://user/cache-test-siblings".to_string();
    let first = format!("{parent}/first");
    let second = format!("{parent}/second");
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
    let path = "applications://user/cache-test-presentation";
    let value = listing(path, vec![item(&format!("{path}/app"), "app", false)]);
    super::super::decoration_cache::store(path, &value, &[]);
    assert!(super::super::decoration_cache::read(path, &value).is_some());
    let mut changed = value.clone();
    changed.entries[0].modified = 42;
    assert!(super::super::decoration_cache::read(path, &changed).is_none());
    store(path, &value);
    drop_subtree(path);
}
