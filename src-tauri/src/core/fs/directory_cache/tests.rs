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
fn only_navigation_enrolls_directories_and_empty_snapshots_are_hits() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().to_str().unwrap();
    std::fs::write(root.path().join("needle.txt"), b"x").unwrap();
    // A background refresh and live list do not create navigation snapshots.
    refresh_tree(path, &|| false).unwrap();
    super::super::trusted(path, "core:test")
        .unwrap()
        .list()
        .unwrap();
    assert!(read(path, true, &[]).unwrap().is_none());
    let mut found = Vec::new();
    search(path, "needle", true, &[], &|| false, &mut |entry| {
        found.push(entry)
    })
    .unwrap();
    assert!(found.is_empty());
    super::super::trusted(path, "core:test")
        .unwrap()
        .desktop_listing(true, &[])
        .unwrap();
    assert_eq!(read(path, true, &[]).unwrap().unwrap().entries.len(), 1);
    let empty = root.path().join("empty");
    std::fs::create_dir(&empty).unwrap();
    let empty = empty.to_str().unwrap();
    store(empty, &listing(empty, vec![]));
    assert!(read(empty, true, &[]).unwrap().unwrap().entries.is_empty());
    drop_subtree(path);
}
#[test]
fn cached_tree_respects_parent_edges_hidden_filters_and_cancellation() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().to_str().unwrap();
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
