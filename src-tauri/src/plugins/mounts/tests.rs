use super::*;
fn test_view(root: &std::path::Path) -> View {
    View {
        descriptor: Descriptor {
            plugin_id: "test.owner".into(),
            contribution_id: "tree".into(),
            view_context: serde_json::json!({"object":"immutable"}),
            resource_root: root.to_string_lossy().into_owned(),
        },
        scope: Scope::new(root).unwrap(),
        nodes: Arc::new(Mutex::new(HashMap::new())),
        created: Instant::now() - std::time::Duration::from_secs(61),
    }
}
#[test]
fn collection_preserves_all_window_references_without_persisting_history() {
    let root = tempfile::tempdir().unwrap();
    let mut state = Registry::default();
    for id in ["tab", "back", "bookmark", "unreferenced"] {
        state.views.insert(id.into(), test_view(root.path()));
    }
    state.references.insert(
        "window-a".into(),
        (HashSet::from(["tab".into(), "back".into()]), HashSet::new()),
    );
    state.references.insert(
        "window-b".into(),
        (HashSet::new(), HashSet::from(["bookmark".into()])),
    );
    collect(&mut state);
    assert_eq!(state.views.len(), 3);
    assert!(!state.views.contains_key("unreferenced"));
    state.references.remove("window-a");
    collect(&mut state);
    assert_eq!(state.views.len(), 1);
    assert!(state.views.contains_key("bookmark"));
    let descriptor = &state.views["bookmark"].descriptor;
    let saved = serde_json::to_value(descriptor).unwrap();
    assert!(saved.get("nodes").is_none());
    assert_eq!(saved["viewContext"]["object"], "immutable");
}
#[test]
fn uri_roundtrip_and_mount_isolation() {
    for name in ["hello world", "中文", "#?%", "a.txt"] {
        let path = uri("abc-123", name).unwrap();
        assert_eq!(parse(&path).unwrap().relative, name);
        assert_eq!(parse(&path).unwrap().mount_id, "abc-123");
    }
    assert_ne!(uri("one", "x").unwrap(), uri("two", "x").unwrap());
}
#[test]
fn malformed_addresses_never_fall_back_to_local_paths() {
    for path in [
        "fiex://other/x/",
        "fiex://mount/x",
        "fiex://mount/x/..",
        "fiex://mount/x/%2e%2e",
        "fiex://mount/x/%252e%252e",
        "fiex://mount/x/a%2fb",
        "fiex://mount/x/a%5cb",
        "fiex://mount/x/%00",
        "fiex://mount/x/%zz",
        "fiex://mount/x/a?b",
        "fiex://mount/x/a//b",
        "FIEX://mount/x/",
    ] {
        assert!(handles(path));
        assert!(parse(path).is_err(), "{path}");
        assert!(reject_local(path).is_err());
    }
    assert!(view("nonexistent-test-mount").is_err());
}
