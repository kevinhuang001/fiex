use super::*;

#[test]
fn rejects_traversal() {
    let owner = OwnerId::plugin("valid.plugin");
    assert!(
        set(
            &OwnerId::Plugin("../evil".into()),
            "global",
            "safe",
            Value::Null
        )
        .is_err()
    );
    assert!(set(&owner, "../x", "safe", Value::Null).is_err());
    assert!(set(&owner, "global", "../x", Value::Null).is_err());
}

#[test]
fn size_cap() {
    let owner = OwnerId::plugin("valid.plugin");
    assert!(
        set(
            &owner,
            "global",
            "test",
            Value::String("a".repeat(MAX_VALUE_BYTES + 1))
        )
        .is_err()
    );
}

#[test]
fn module_owned_value_quota_does_not_raise_plugin_limit() {
    let applications = OwnerId::core(CoreOwnerKey::new("quota-test").with_value_limit(
        "tree",
        "state",
        MAX_SCOPE_BYTES - 1024,
    ));
    let plugin = OwnerId::plugin("valid.plugin");
    assert_eq!(
        max_value_bytes(&applications, "tree", "state"),
        MAX_SCOPE_BYTES - 1024
    );
    assert_eq!(max_value_bytes(&plugin, "tree", "state"), MAX_VALUE_BYTES);
    assert_eq!(
        max_value_bytes(&applications, "other", "state"),
        MAX_VALUE_BYTES
    );
}

#[test]
fn cas_distinguishes_absent_null_and_conflicts_and_skips_noop_saves() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("state.json");
    let mut state = Snapshot::new(path.clone());
    assert!(
        state
            .compare_and_set("key", None, Some(Value::Null))
            .unwrap()
    );
    assert!(
        !state
            .compare_and_set("key", None, Some(Value::Bool(true)))
            .unwrap()
    );
    assert!(
        state
            .compare_and_set("key", Some(&Value::Null), Some(Value::from(1)))
            .unwrap()
    );
    assert!(
        !state
            .compare_and_set("key", Some(&Value::Null), None)
            .unwrap()
    );
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    assert!(
        state
            .compare_and_set("key", Some(&Value::from(1)), Some(Value::from(1)))
            .unwrap()
    );
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
    assert_eq!(state.get("key").unwrap(), Some(Value::from(1)));
}

#[test]
fn set_equal_and_delete_missing_do_not_write() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("state.json");
    let mut state = Snapshot::new(path.clone());
    state.set("key", Value::from(1)).unwrap();
    let before = fs::metadata(&path).unwrap().modified().unwrap();
    state.set("key", Value::from(1)).unwrap();
    state.delete("absent").unwrap();
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), before);
}

#[test]
fn scope_cache_is_bounded_and_keeps_scope_locks_separate() {
    let mut cache = ScopeCache::default();
    for index in 0..(MAX_CACHED_SCOPES + 1) {
        drop(
            cache
                .get_or_create(
                    StorageKey {
                        owner: OwnerId::plugin("valid.plugin"),
                        scope: format!("scope{index}"),
                    },
                    PathBuf::from(format!("/tmp/fiex-storage-{index}.json")),
                )
                .unwrap(),
        );
    }
    assert_eq!(cache.entries.len(), MAX_CACHED_SCOPES);
    let first = cache
        .get_or_create(
            StorageKey {
                owner: OwnerId::plugin("valid.plugin"),
                scope: "one".into(),
            },
            PathBuf::from("/tmp/fiex-storage-one.json"),
        )
        .unwrap();
    let second = cache
        .get_or_create(
            StorageKey {
                owner: OwnerId::plugin("valid.plugin"),
                scope: "two".into(),
            },
            PathBuf::from("/tmp/fiex-storage-two.json"),
        )
        .unwrap();
    let _guard = first.lock().unwrap();
    assert!(second.try_lock().is_ok());
}

#[test]
fn concurrent_compare_updates_do_not_lose_increments() {
    let temp = tempfile::tempdir().unwrap();
    let snapshot = Arc::new(Mutex::new(Snapshot::new(temp.path().join("state.json"))));
    let workers = (0..4)
        .map(|_| {
            let snapshot = Arc::clone(&snapshot);
            std::thread::spawn(move || {
                for _ in 0..20 {
                    loop {
                        let current = snapshot.lock().unwrap().get("count").unwrap();
                        let next =
                            Value::from(current.as_ref().and_then(Value::as_u64).unwrap_or(0) + 1);
                        if snapshot
                            .lock()
                            .unwrap()
                            .compare_and_set("count", current.as_ref(), Some(next))
                            .unwrap()
                        {
                            break;
                        }
                    }
                }
            })
        })
        .collect::<Vec<_>>();
    for worker in workers {
        worker.join().unwrap();
    }
    assert_eq!(
        snapshot.lock().unwrap().get("count").unwrap(),
        Some(Value::from(80))
    );
}
