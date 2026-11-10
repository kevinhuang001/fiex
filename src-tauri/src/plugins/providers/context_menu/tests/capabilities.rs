use super::*;
use serde_json::Value;

fn action_items(ctx: &super::builtin::Context, id: &str) -> Vec<Value> {
    if !super::builtin::providers(ctx)
        .iter()
        .any(|provider| provider.id == id)
    {
        return Vec::new();
    }
    super::builtin::resolve(id, ctx).unwrap_or_else(|error| panic!("{id}: {error}"))["items"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn each_common_action_has_its_own_provider_and_unavailable_actions_are_omitted() {
    let path = "fiex://mount/gating-mount/report.txt";
    let ctx = mount_selection(
        path,
        false,
        fiex_resource::FileCapabilities {
            read: true,
            stat: true,
            ..Default::default()
        },
    );
    for (provider_id, item_id) in [
        ("builtin:open", "open"),
        ("builtin:preview", "preview"),
        ("builtin:info", "info"),
        ("builtin:copy", "copy"),
        ("builtin:copy-path", "copyPath"),
    ] {
        let items = action_items(&ctx, provider_id);
        assert_eq!(items.len(), 1, "{provider_id}");
        assert_eq!(items[0]["id"], item_id);
    }
    assert!(action_items(&ctx, "builtin:duplicate").is_empty());
    assert!(action_items(&ctx, "builtin:delete-permanently").is_empty());
    assert!(action_items(&ctx, "builtin:move-to-trash").is_empty());
    assert!(action_items(&ctx, "builtin:cut").is_empty());
    assert_never_disabled(&ctx);
}

#[test]
fn cut_keeps_the_existing_read_and_remove_requirements() {
    let path = "/tmp/cut-capability-test";
    for (read, remove, expected) in [
        (false, true, false),
        (true, false, false),
        (true, true, true),
    ] {
        let context = super::builtin::Context::for_test_caps(
            "/tmp",
            vec![path.into()],
            vec![mount_entry(path, false)],
            fiex_resource::FileCapabilities {
                read,
                remove,
                ..Default::default()
            },
        );
        assert_eq!(!action_items(&context, "builtin:cut").is_empty(), expected);
    }
}

#[test]
fn generic_open_does_not_require_read_and_preview_keeps_its_read_rule() {
    let path = "applications://System%20App";
    let mut application = mount_entry(path, false);
    application.link_target = Some("stable-system-locator".into());
    let ctx = super::builtin::Context::for_test_caps(
        "applications://",
        vec![path.into()],
        vec![application],
        fiex_resource::FileCapabilities {
            stat: true,
            ..Default::default()
        },
    );
    assert_eq!(action_items(&ctx, "builtin:open")[0]["id"], "open");
    assert!(action_items(&ctx, "builtin:info").is_empty());
    assert!(action_items(&ctx, "builtin:copy-path").is_empty());
    assert!(action_items(&ctx, "builtin:preview").is_empty());
}

#[test]
fn trash_and_remove_are_independent_capabilities_and_both_can_be_shown() {
    let path = "/tmp/atomic-delete-test";
    for (trash, remove, expected) in [
        (false, false, [false, false]),
        (true, false, [true, false]),
        (false, true, [false, true]),
        (true, true, [true, true]),
    ] {
        let ctx = super::builtin::Context::for_test_caps(
            "/tmp",
            vec![path.into()],
            vec![mount_entry(path, false)],
            fiex_resource::FileCapabilities {
                trash,
                remove,
                ..Default::default()
            },
        );
        assert_eq!(
            [
                !action_items(&ctx, "builtin:move-to-trash").is_empty(),
                !action_items(&ctx, "builtin:delete-permanently").is_empty(),
            ],
            expected
        );
        assert_never_disabled(&ctx);
    }
}

#[test]
fn blank_canvas_actions_use_independent_capabilities_and_view_excludes() {
    let capabilities = fiex_resource::FileCapabilities {
        read: true,
        write: true,
        create: true,
        ..Default::default()
    };
    let local = super::builtin::Context::for_test_caps("/tmp/blank", vec![], vec![], capabilities);
    assert_eq!(
        action_items(&local, "builtin:new-folder")[0]["id"],
        "newFolder"
    );
    let new_file = action_items(&local, "builtin:new-file");
    assert_eq!(new_file[0]["id"], "newFile");
    assert_eq!(new_file[0]["kind"], "submenu");
    assert_eq!(new_file[1]["id"], "newFile.empty");
    assert_eq!(new_file[2]["id"], "newFile.office");
    assert_eq!(new_file[3]["id"], "newFile.docx");
    assert_eq!(new_file[4]["id"], "newFile.xlsx");
    assert_eq!(new_file[5]["id"], "newFile.pptx");

    let apps = super::builtin::Context::for_test_caps(
        "applications://",
        vec![],
        vec![],
        fiex_resource::FileCapabilities::ALL,
    );
    assert_eq!(
        action_items(&apps, "builtin:new-folder")[0]["id"],
        "newFolder"
    );
    assert!(action_items(&apps, "builtin:new-file").is_empty());

    for view in ["recent://", "trash://"] {
        let context = super::builtin::Context::for_test_caps(
            view,
            vec![],
            vec![],
            fiex_resource::FileCapabilities::ALL,
        );
        assert!(action_items(&context, "builtin:new-folder").is_empty());
        assert!(action_items(&context, "builtin:new-file").is_empty());
    }
}

#[test]
fn trash_view_reuses_generic_remove_and_recent_never_inherits_source_delete() {
    let trash_path = "trash://opaque-item";
    let trash = super::builtin::Context::for_test_caps(
        "trash://",
        vec![trash_path.into()],
        vec![mount_entry(trash_path, false)],
        fiex_resource::FileCapabilities {
            remove: true,
            ..Default::default()
        },
    );
    assert_eq!(
        action_items(&trash, "builtin:delete-permanently")[0]["id"],
        "delete"
    );
    assert!(action_items(&trash, "builtin:move-to-trash").is_empty());

    let recent_path = "recent://Recent%20file.txt?id=opaque-id";
    let recent = super::builtin::Context::for_test_caps(
        "recent://",
        vec![recent_path.into()],
        vec![mount_entry(recent_path, false)],
        fiex_resource::FileCapabilities {
            read: true,
            remove: true,
            trash: true,
            ..Default::default()
        },
    );
    assert!(action_items(&recent, "builtin:delete-permanently").is_empty());
    assert!(action_items(&recent, "builtin:move-to-trash").is_empty());
}

#[test]
fn folder_selections_offer_copy_cut_and_sibling_duplicate() {
    let destination = tempfile::tempdir().unwrap();
    let folder = destination
        .path()
        .join("folder")
        .to_string_lossy()
        .into_owned();
    let entry = crate::core::files::Entry {
        path: folder.clone(),
        is_dir: true,
        ..Default::default()
    };
    let context = super::builtin::Context::for_test(
        destination.path().to_str().unwrap(),
        vec![folder],
        vec![entry],
    );

    for (provider_id, item_id) in [
        ("builtin:copy", "copy"),
        ("builtin:cut", "cut"),
        ("builtin:duplicate", "duplicate"),
    ] {
        assert_eq!(action_items(&context, provider_id)[0]["id"], item_id);
    }
}

#[test]
fn folder_duplicate_is_hidden_when_the_destination_cannot_create_directories() {
    let folder = "/tmp/fiex-folder-duplicate-source";
    let context = super::builtin::Context::for_test_caps(
        "/tmp",
        vec![folder.into()],
        vec![mount_entry(folder, true)],
        fiex_resource::FileCapabilities {
            list: true,
            remove: true,
            ..Default::default()
        },
    );

    assert_eq!(action_items(&context, "builtin:copy").len(), 1);
    assert_eq!(action_items(&context, "builtin:cut").len(), 1);
    assert!(action_items(&context, "builtin:duplicate").is_empty());
}
