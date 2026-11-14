use super::*;

#[test]
fn interrupted_metadata_replace_recovers_last_committed_cache_state() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("remote-meta.json");

    let mut committed = RemoteMeta::default();
    committed.files.insert(
        "docs/notes.txt".into(),
        RemoteMetaFile {
            downloaded: true,
            pinned: true,
            revision: "remote-v1".into(),
            baseline: "content-v1".into(),
            local_stamp: "stamp-v1".into(),
            ..Default::default()
        },
    );
    committed
        .listed_children
        .insert("docs".into(), vec!["docs/notes.txt".into()]);
    save_meta_to(&path, &committed).unwrap();

    // Model a process stopping after writing its sibling staging file but
    // before the atomic rename that commits the new metadata.
    let orphaned_stage = directory.path().join(".remote-meta.json-interrupted.tmp");
    fs::write(
        &orphaned_stage,
        br#"{"files":{"docs/notes.txt":{"downloaded":true"#,
    )
    .unwrap();

    // A new process reads only the committed path, preserving the cache
    // baseline and pin intent from before the interrupted replacement.
    let mut recovered = load_meta_from(&path);
    let file = recovered.files.get("docs/notes.txt").unwrap();
    assert!(file.downloaded);
    assert!(file.pinned);
    assert_eq!(file.revision, "remote-v1");
    assert_eq!(file.baseline, "content-v1");
    assert_eq!(file.local_stamp, "stamp-v1");
    assert_eq!(
        recovered.listed_children.get("docs").unwrap(),
        &["docs/notes.txt"]
    );
    assert!(orphaned_stage.exists());

    // A retry atomically replaces the old snapshot, and the next restart
    // observes the fully committed new baseline rather than either partial file.
    let file = recovered.files.get_mut("docs/notes.txt").unwrap();
    file.revision = "remote-v2".into();
    file.baseline = "content-v2".into();
    save_meta_to(&path, &recovered).unwrap();

    let recovered = load_meta_from(&path);
    let file = recovered.files.get("docs/notes.txt").unwrap();
    assert_eq!(file.revision, "remote-v2");
    assert_eq!(file.baseline, "content-v2");
    assert!(file.downloaded);
    assert!(file.pinned);
}
