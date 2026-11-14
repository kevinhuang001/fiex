use super::*;
#[test]
fn subtree_boundaries() {
    assert!(in_tree("a/b", "a"));
    assert!(!in_tree("ab/file", "a"));
    assert!(in_tree("anything", ""));
}
#[test]
fn fingerprints_include_content() {
    let file = tempfile::NamedTempFile::new().unwrap();
    fs::write(file.path(), b"one").unwrap();
    let first = fingerprint(file.path()).unwrap();
    fs::write(file.path(), b"two").unwrap();
    assert_ne!(first, fingerprint(file.path()).unwrap());
}

#[test]
fn uncommitted_write_lease_aborts_but_finished_lease_does_not() {
    let calls = std::cell::RefCell::new(Vec::new());
    let abort = |plugin: &str, mount: &str, stream: &str| {
        calls
            .borrow_mut()
            .push((plugin.to_string(), mount.to_string(), stream.to_string()));
    };
    {
        let lease = WriteLease {
            plugin: "plugin",
            mount_id: "mount",
            stream: "failed-stream",
            finished: false,
            abort: &abort,
        };
        drop(lease);
    }
    {
        let lease = WriteLease {
            plugin: "plugin",
            mount_id: "mount",
            stream: "committed-stream",
            finished: true,
            abort: &abort,
        };
        drop(lease);
    }
    assert_eq!(
        *calls.borrow(),
        [("plugin".into(), "mount".into(), "failed-stream".into())]
    );
}

fn connection() -> StoredConnection {
    StoredConnection {
        id: "reconcile-test".into(),
        mount_id: "mount-reconcile-test".into(),
        cache_revision: "revision-1".into(),
        name: "Reconcile test".into(),
        kind: "org.example.remote".into(),
        plugin_id: "org.example.remote".into(),
        settings: Default::default(),
        secret_keys: Vec::new(),
        saved: true,
        autostart: false,
    }
}

fn downloaded_meta(local: &std::path::Path) -> (RemoteMeta, RemoteMetaFile) {
    let file = RemoteMetaFile {
        downloaded: true,
        revision: "remote-v1".into(),
        baseline: fingerprint(local).unwrap(),
        local_stamp: stamp(local).unwrap(),
        size: fs::metadata(local).unwrap().len(),
        node_id: "opaque-node".into(),
        ..Default::default()
    };
    let mut meta = RemoteMeta::default();
    meta.files.insert("document.txt".into(), file.clone());
    (meta, file)
}

fn persisted_meta(meta: &RemoteMeta) -> RemoteMeta {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("remote-meta.json");
    save_meta_to(&path, meta).unwrap();
    load_meta_from(&path)
}

#[test]
fn disconnected_stat_keeps_the_cached_bytes_and_baseline_for_retry() {
    let conn = connection();
    let local = tempfile::NamedTempFile::new().unwrap();
    fs::write(local.path(), b"local baseline").unwrap();
    let original = fs::read(local.path()).unwrap();
    let (mut meta, file) = downloaded_meta(local.path());
    let baseline = file.baseline.clone();
    let mut changes = Vec::new();

    let outcome = cached_file::reconcile_cached_file_with(
        cached_file::ReconcileContext {
            conn: &conn,
            rel: "document.txt",
            pull: false,
            changes: &mut changes,
            meta: &mut meta,
            local: local.path(),
            file,
        },
        cached_file::ReconcileIo {
            stat: |_: &StoredConnection, _: &str, _: &RemoteMetaFile| Err("offline".into()),
            fetch: |_: &StoredConnection, _: &str, _: &std::path::Path| {
                panic!("offline stat must stop before download")
            },
            upload: |_: &StoredConnection, _: &str, _: &RemoteMetaFile| {
                panic!("offline stat must stop before upload")
            },
        },
    );

    assert_eq!(outcome.result.unwrap_err(), "offline");
    assert!(!outcome.metadata_changed);
    assert_eq!(fs::read(local.path()).unwrap(), original);
    assert_eq!(meta.files["document.txt"].baseline, baseline);
    assert_eq!(meta.files["document.txt"].revision, "remote-v1");
    assert!(!meta.files["document.txt"].conflict);
    assert!(changes.is_empty());
    let recovered = persisted_meta(&meta);
    assert_eq!(recovered.files["document.txt"].baseline, baseline);
    assert_eq!(recovered.files["document.txt"].revision, "remote-v1");
}

#[test]
fn failed_remote_refresh_preserves_the_cached_copy_and_baseline() {
    let conn = connection();
    let local = tempfile::NamedTempFile::new().unwrap();
    fs::write(local.path(), b"cached v1").unwrap();
    let original = fs::read(local.path()).unwrap();
    let (mut meta, file) = downloaded_meta(local.path());
    let baseline = file.baseline.clone();
    let mut changes = Vec::new();

    let outcome = cached_file::reconcile_cached_file_with(
        cached_file::ReconcileContext {
            conn: &conn,
            rel: "document.txt",
            pull: true,
            changes: &mut changes,
            meta: &mut meta,
            local: local.path(),
            file,
        },
        cached_file::ReconcileIo {
            stat: |_: &StoredConnection, _: &str, _: &RemoteMetaFile| {
                Ok(json!({"revision":"remote-v2"}))
            },
            fetch: |_: &StoredConnection, _: &str, _: &std::path::Path| {
                Err("offline during refresh".into())
            },
            upload: |_: &StoredConnection, _: &str, _: &RemoteMetaFile| {
                panic!("changed remote revision must refresh before upload")
            },
        },
    );

    assert_eq!(outcome.result.unwrap_err(), "offline during refresh");
    assert!(!outcome.metadata_changed);
    assert_eq!(fs::read(local.path()).unwrap(), original);
    assert_eq!(meta.files["document.txt"].baseline, baseline);
    assert_eq!(meta.files["document.txt"].revision, "remote-v1");
    assert!(changes.is_empty());
    let recovered = persisted_meta(&meta);
    assert_eq!(recovered.files["document.txt"].baseline, baseline);
    assert_eq!(recovered.files["document.txt"].revision, "remote-v1");
}

#[test]
fn local_and_remote_edits_mark_a_conflict_without_replacing_local_bytes() {
    let conn = connection();
    let local = tempfile::NamedTempFile::new().unwrap();
    fs::write(local.path(), b"cached v1").unwrap();
    let (mut meta, file) = downloaded_meta(local.path());
    fs::write(local.path(), b"local edit").unwrap();
    let edited = fs::read(local.path()).unwrap();
    let mut changes = Vec::new();

    let outcome = cached_file::reconcile_cached_file_with(
        cached_file::ReconcileContext {
            conn: &conn,
            rel: "document.txt",
            pull: true,
            changes: &mut changes,
            meta: &mut meta,
            local: local.path(),
            file,
        },
        cached_file::ReconcileIo {
            stat: |_: &StoredConnection, _: &str, _: &RemoteMetaFile| {
                Ok(json!({"revision":"remote-v2"}))
            },
            fetch: |_: &StoredConnection, _: &str, _: &std::path::Path| {
                panic!("a conflict must not replace the local file")
            },
            upload: |_: &StoredConnection, _: &str, _: &RemoteMetaFile| {
                panic!("a conflict must not upload automatically")
            },
        },
    );

    assert!(outcome.result.is_err());
    assert!(outcome.metadata_changed);
    assert!(meta.files["document.txt"].conflict);
    assert_eq!(fs::read(local.path()).unwrap(), edited);
    assert_eq!(changes.len(), 1);
}
