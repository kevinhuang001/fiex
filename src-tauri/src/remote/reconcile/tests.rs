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
