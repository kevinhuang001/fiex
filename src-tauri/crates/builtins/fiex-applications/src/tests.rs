use super::*;

fn folder(id: &str, parent: Option<&str>) -> VirtualFolder {
    VirtualFolder {
        id: id.into(),
        parent: parent.map(str::to_owned),
        name: id.into(),
        order: 0,
    }
}

#[test]
fn store_validates_updates_and_advances_revision_once() {
    let storage = fiex_storage::native::OwnerBoundStorage::from_registry(CoreOwnerKey::new(
        "applications-store-tests",
    ))
    .scoped("tree")
    .unwrap();
    storage.delete("state").unwrap();
    let store = ApplicationsStore::new(storage.clone());

    let initial = store.snapshot().unwrap();
    assert_eq!(initial.schema, SCHEMA);
    assert_eq!(initial.revision, 0);

    let updated = store
        .update(|state| {
            state
                .folders
                .insert("root-folder".into(), folder("root-folder", None));
            Ok(())
        })
        .unwrap();
    assert_eq!(updated.revision, 1);

    let unchanged = store.update(|_| Ok(())).unwrap();
    assert_eq!(unchanged, updated);

    assert!(store
        .update(|state| {
            state.folders.insert(
                "orphan-folder".into(),
                folder("orphan-folder", Some("missing-parent")),
            );
            Ok(())
        })
        .is_err());
    assert_eq!(store.snapshot().unwrap(), updated);
    storage.delete("state").unwrap();
}

#[test]
fn virtual_identity_is_opaque_and_independent_of_display_name() {
    let id = "folder-0123456789abcdef";
    let before = folder_uri(id);
    let mut node = folder(id, None);
    node.name = "Design".into();
    let after_rename = folder_uri(&node.id);
    assert_eq!(before, after_rename);
    assert_eq!(folder_id(&before).as_deref(), Some(id));
    assert!(folder_id("applications://folder_../escape").is_none());
}

#[test]
fn virtual_paths_follow_the_visible_folder_tree_without_type_directories() {
    let mut state = ApplicationsState {
        schema: SCHEMA,
        ..Default::default()
    };
    state
        .folders
        .insert("root-folder".into(), folder("root-folder", None));
    state.folders.insert(
        "child-folder".into(),
        folder("child-folder", Some("root-folder")),
    );
    state.apps.insert(
        "root-app".into(),
        ApplicationNode {
            id: "root-app".into(),
            parent: None,
            display_name: "Root app".into(),
            source: ApplicationSource::System {
                stable_locator: "root-app-locator".into(),
            },
            order: 0,
            unavailable: false,
        },
    );
    state.apps.insert(
        "nested-app".into(),
        ApplicationNode {
            id: "nested-app".into(),
            parent: Some("child-folder".into()),
            display_name: "Nested app".into(),
            source: ApplicationSource::System {
                stable_locator: "nested-app-locator".into(),
            },
            order: 0,
            unavailable: false,
        },
    );
    assert_eq!(
        folder_uri_in(&state, "root-folder").as_deref(),
        Some("applications://folder_root-folder")
    );
    assert_eq!(
        folder_uri_in(&state, "child-folder").as_deref(),
        Some("applications://folder_root-folder/folder_child-folder")
    );
    assert_eq!(
        app_uri_in(&state, "root-app").as_deref(),
        Some("applications://app_root-app")
    );
    assert_eq!(
        app_uri_in(&state, "nested-app").as_deref(),
        Some("applications://folder_root-folder/folder_child-folder/app_nested-app")
    );
    assert!(app_id("applications://app_root-app/app_nested-app").is_none());
}

#[test]
fn state_validation_rejects_cycles_and_missing_parents() {
    let mut state = ApplicationsState {
        schema: SCHEMA,
        ..Default::default()
    };
    state
        .folders
        .insert("folder-a".into(), folder("folder-a", Some("folder-b")));
    state
        .folders
        .insert("folder-b".into(), folder("folder-b", Some("folder-a")));
    assert!(validate_state(&state).unwrap_err().contains("cycle"));

    let mut state = ApplicationsState {
        schema: SCHEMA,
        ..Default::default()
    };
    state
        .folders
        .insert("folder-a".into(), folder("folder-a", Some("missing")));
    assert!(validate_state(&state).unwrap_err().contains("parent"));
}

#[test]
fn launchable_application_suffixes_are_accepted() {
    for path in [
        r"C:\Program Files\App\app.exe",
        r"C:\Program Files\App\tool.cmd",
        r"C:\Windows\System32\app.msc",
        r"C:\Users\me\AppData\Roaming\Microsoft\Installer\{ABC}\app.appref-ms",
    ] {
        assert!(is_executable_target(path), "{path} should be accepted");
    }
}

#[test]
fn documents_and_websites_are_not_application_targets() {
    for path in [
        r"C:\Program Files\App\readme.pdf",
        r"C:\Program Files\App\manual.docx",
        "https://example.com/download",
        "",
    ] {
        assert!(!is_executable_target(path), "{path} should be rejected");
    }
}
