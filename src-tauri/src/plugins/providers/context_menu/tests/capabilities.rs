use super::*;

#[test]
fn a_read_only_mount_omits_every_mutating_group() {
    let ctx = mount_selection(
        "fiex://mount/gating-mount/report.txt",
        false,
        crate::plugins::resource::file_capabilities::FileCapabilities {
            read: true,
            stat: true,
            ..Default::default()
        },
    );
    assert_eq!(
        provider_ids(&ctx),
        ["builtin:opening", "builtin:clipboard", "builtin:copy-path"]
    );
    let opening = super::builtin::resolve("builtin:opening", &ctx).unwrap();
    let items = opening["items"].as_array().unwrap();
    assert!(items.iter().any(|item| item["id"] == "open"));
    assert!(items.iter().any(|item| item["id"] == "preview"));
    assert!(items.iter().any(|item| item["id"] == "info"));
    // Read-only still copies, but can neither cut nor duplicate.
    let clipboard = super::builtin::resolve("builtin:clipboard", &ctx).unwrap();
    let items = clipboard["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], "copy");
    // Unavailable groups are absent from the registered candidate list above.
    assert_never_disabled(&ctx);
}

#[test]
fn an_unreadable_file_hides_open_but_keeps_info() {
    let ctx = mount_selection(
        "fiex://mount/gating-mount/report.txt",
        false,
        crate::plugins::resource::file_capabilities::FileCapabilities::default(),
    );
    let opening = super::builtin::resolve("builtin:opening", &ctx).unwrap();
    let items = opening["items"].as_array().unwrap();
    assert!(!items.iter().any(|item| item["id"] == "open"));
    assert!(!items.iter().any(|item| item["id"] == "preview"));
    assert!(items.iter().any(|item| item["id"] == "info"));
    assert_never_disabled(&ctx);
}

#[test]
fn a_directory_without_read_still_opens_by_navigation() {
    let ctx = mount_selection(
        "fiex://mount/gating-mount/folder",
        true,
        crate::plugins::resource::file_capabilities::FileCapabilities::default(),
    );
    let opening = super::builtin::resolve("builtin:opening", &ctx).unwrap();
    let items = opening["items"].as_array().unwrap();
    assert!(items.iter().any(|item| item["id"] == "open"));
    assert!(!items.iter().any(|item| item["id"] == "preview"));
    assert!(items.iter().any(|item| item["id"] == "info"));
    assert_never_disabled(&ctx);
}

#[test]
fn remove_adds_trash_and_delete_forever() {
    let capabilities = crate::plugins::resource::file_capabilities::FileCapabilities {
        read: true,
        remove: true,
        ..Default::default()
    };
    let ctx = mount_selection("fiex://mount/gating-mount/a.txt", false, capabilities);
    assert!(provider_ids(&ctx).contains(&"builtin:trash".to_string()));
    let trash = super::builtin::resolve("builtin:trash", &ctx).unwrap();
    // Mounts have no Trash: the provider's `remove` is a permanent delete.
    assert_eq!(trash["items"][0]["id"], "delete");
    assert_eq!(trash["items"][0]["placement"], "tail");
    assert_eq!(trash["items"][0]["danger"], true);
    // A VFS remove capability never implies restore or empty-the-bin.
    let items = trash["items"].as_array().unwrap();
    assert!(items
        .iter()
        .all(|item| item["id"] != "restore" && item["id"] != "emptyTrash"));
    // A write-only mount keeps the trash group out.
    let without_remove = mount_selection(
        "fiex://mount/gating-mount/a.txt",
        false,
        crate::plugins::resource::file_capabilities::FileCapabilities {
            read: true,
            write: true,
            ..Default::default()
        },
    );
    assert!(!provider_ids(&without_remove).contains(&"builtin:trash".to_string()));
}

#[test]
fn create_capability_only_adds_blank_canvas_actions() {
    let capabilities = crate::plugins::resource::file_capabilities::FileCapabilities {
        read: true,
        stat: true,
        list: true,
        write: true,
        create: true,
        rename: true,
        remove: false,
        ..Default::default()
    };
    let ctx = mount_selection("fiex://mount/gating-mount/a.txt", false, capabilities);
    let ids = provider_ids(&ctx);
    assert!(ids.contains(&"builtin:rename".to_string()));
    assert!(ids.contains(&"builtin:clipboard".to_string()));
    let emitted = emitted_items(&ctx);
    assert!(
        emitted.iter().all(|item| item["id"] != "newFolder"),
        "{emitted:?}"
    );
    let clipboard = super::builtin::resolve("builtin:clipboard", &ctx).unwrap();
    let items = clipboard["items"].as_array().unwrap();
    for id in ["copy", "duplicate"] {
        assert!(items.iter().any(|item| item["id"] == id), "{id}");
    }
    assert_never_disabled(&ctx);
    // Selected items never receive blank-canvas actions.
    let without_create = super::builtin::Context::for_test_caps(
        "fiex://mount/gating-mount",
        vec!["fiex://mount/gating-mount/a.txt".into()],
        vec![mount_entry("fiex://mount/gating-mount/a.txt", false)],
        crate::plugins::resource::file_capabilities::FileCapabilities {
            read: true,
            rename: true,
            ..Default::default()
        },
    );
    assert_eq!(provider_ids(&ctx), provider_ids(&without_create));
    assert_never_disabled(&without_create);
}

#[test]
fn a_read_only_directory_drops_mutating_items_entirely() {
    // A capability snapshot is not enough: the directory flag removes them too.
    let ctx = super::builtin::Context::for_test_caps(
        "fiex://mount/gating-mount",
        vec!["fiex://mount/gating-mount/a.txt".into()],
        vec![mount_entry("fiex://mount/gating-mount/a.txt", false)],
        crate::plugins::resource::file_capabilities::FileCapabilities {
            read: true,
            stat: true,
            ..Default::default()
        },
    );
    let ids = provider_ids(&ctx);
    for id in ["builtin:rename", "builtin:batch-rename", "builtin:trash"] {
        assert!(!ids.contains(&id.to_string()), "{id}");
    }
    assert_never_disabled(&ctx);
}

#[test]
fn blank_selection_offers_only_capability_gated_create_actions() {
    let capabilities = crate::plugins::resource::file_capabilities::FileCapabilities {
        read: true,
        write: true,
        create: true,
        ..Default::default()
    };
    let ctx = super::builtin::Context::for_test_caps("/tmp/blank", vec![], vec![], capabilities);
    assert_eq!(
        provider_ids(&ctx),
        ["builtin:new-file", "builtin:new-folder"]
    );
    let folder = super::builtin::resolve("builtin:new-folder", &ctx).unwrap();
    assert_eq!(folder["items"][0]["id"], "newFolder");
    let file = super::builtin::resolve("builtin:new-file", &ctx).unwrap();
    assert_eq!(file["items"][0]["id"], "newFile");
    assert_never_disabled(&ctx);

    let files_only = super::builtin::Context::for_test_caps(
        "/tmp/blank",
        vec![],
        vec![],
        crate::plugins::resource::file_capabilities::FileCapabilities {
            read: true,
            write: true,
            ..Default::default()
        },
    );
    let only = super::builtin::resolve("builtin:new-file", &files_only).unwrap();
    assert_eq!(only["items"][0]["id"], "newFile");
    assert!(!provider_ids(&files_only).contains(&"builtin:new-folder".to_string()));

    let empty = super::builtin::Context::for_test_caps(
        "/tmp/blank",
        vec![],
        vec![],
        crate::plugins::resource::file_capabilities::FileCapabilities::default(),
    );
    assert!(!provider_ids(&empty).contains(&"builtin:new-folder".to_string()));
    assert!(!provider_ids(&empty).contains(&"builtin:new-file".to_string()));
}

#[test]
fn applications_menu_excludes_generic_file_actions() {
    let path = "applications://c3lzdGVtLWFwcA";
    let ctx = super::builtin::Context::for_test_caps(
        "applications://",
        vec![path.into()],
        vec![mount_entry(path, false)],
        crate::plugins::resource::file_capabilities::FileCapabilities {
            read: true,
            stat: true,
            ..Default::default()
        },
    );
    let ids = provider_ids(&ctx);
    assert!(ids.contains(&"core.applications.menu".to_string()));
    for id in [
        "builtin:opening",
        "builtin:copy-path",
        "builtin:clipboard",
        "builtin:rename",
        "builtin:trash",
    ] {
        assert!(!ids.contains(&id.to_string()), "{id}");
    }
    let native = super::builtin::resolve("core.applications.menu", &ctx).unwrap();
    assert_eq!(native["items"][0]["id"], "open");
    assert!(super::builtin::resolve("builtin:opening", &ctx).is_err());
    assert!(super::builtin::execute("core.applications.menu", "delete", &ctx).is_err());
}

#[test]
fn applications_blank_canvas_uses_exclude_to_keep_only_new_folder() {
    let ctx = super::builtin::Context::for_test_caps(
        "applications://",
        vec![],
        vec![],
        crate::plugins::resource::file_capabilities::FileCapabilities::ALL,
    );
    let ids = provider_ids(&ctx);
    assert!(ids.contains(&"builtin:new-folder".to_string()));
    assert!(!ids.contains(&"builtin:new-file".to_string()));
    assert_eq!(
        super::builtin::resolve("builtin:new-folder", &ctx).unwrap()["items"][0]["id"],
        "newFolder"
    );
    assert!(super::builtin::resolve("builtin:new-file", &ctx).is_err());
}

#[test]
fn trash_and_recent_blank_canvases_exclude_generic_create_actions() {
    let capabilities = crate::plugins::resource::file_capabilities::FileCapabilities::ALL;
    for directory in ["trash://", "recent://"] {
        let ctx = super::builtin::Context::for_test_caps(directory, vec![], vec![], capabilities);
        let ids = provider_ids(&ctx);
        assert!(
            !ids.contains(&"builtin:new-folder".to_string()),
            "{directory}"
        );
        assert!(
            !ids.contains(&"builtin:new-file".to_string()),
            "{directory}"
        );
    }
}
