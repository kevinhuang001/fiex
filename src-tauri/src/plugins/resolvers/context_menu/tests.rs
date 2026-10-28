use super::validation::*;
use serde_json::json;

#[test]
fn menu_rejects_nested_legacy_fields_and_unknown_parent() {
    assert!(menu_response_valid(&json!({"items": [
        {"id":"root","kind":"submenu","title":"Root"},
        {"id":"open","kind":"action","title":"Open","parentId":"root"}
    ]}))
    .unwrap()
    .contains("open"));
    for invalid in [
        json!({"items":[{"id":"a","title":"A","kind":"action","when":"true"}]}),
        json!({"items":[{"id":"a","title":"A","kind":"action","children":[]}]}),
        json!({"items":[{"id":"a","title":"A","kind":"action","parentId":"missing"}]}),
        json!({"items":[{"id":"a","title":"A","kind":"submenu","parentId":"a"}]}),
        json!({"items":[{"id":"a","title":"A","kind":"action"},{"id":"a","title":"B","kind":"action"}]}),
    ] {
        assert!(menu_response_valid(&invalid).is_err());
    }
}

#[test]
fn dynamic_children_require_a_valid_parent_and_cannot_forge_ids() {
    let response = serde_json::json!({
        "items":[{"id":"child","kind":"action","title":"Child"}]
    });
    let validated = validated_submenu(&response, "root", 1).unwrap();
    assert_eq!(validated.items[0]["parentId"], "root");
    assert!(validated.actions.contains("child"));
    assert!(validated.submenus.is_empty());
    assert!(validated_submenu(&response, "root", 4).is_err());
    let spoof = serde_json::json!({"items":[{
        "id":"child","kind":"action","title":"Child","parentId":"other"
    }]});
    assert!(validated_submenu(&spoof, "root", 1).is_err());
}

#[test]
fn root_menu_requires_lazy_submenu_children() {
    let nested = serde_json::json!({"items":[
        {"id":"group","kind":"submenu","title":"Group"},
        {"id":"run","kind":"action","title":"Run","parentId":"group"}
    ]});
    assert!(root_menu_valid(&nested).is_err());
    let root = serde_json::json!({"items":[
        {"id":"group","kind":"submenu","title":"Group"}
    ]});
    assert!(root_menu_valid(&root).unwrap().is_empty());
}

#[test]
fn submenu_cannot_be_executed_as_an_action() {
    let valid = menu_response_valid(&json!({"items":[
        {"id":"group","title":"Group","kind":"submenu"},
        {"id":"act","title":"Act","kind":"action","parentId":"group"}
    ]}))
    .unwrap();
    assert!(!valid.contains("group"));
    assert!(valid.contains("act"));
}

fn temp_selection() -> (tempfile::TempDir, String, Vec<String>) {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().to_string_lossy().into_owned();
    let paths = ["one.txt", "two.txt"]
        .map(|name| {
            let path = temp.path().join(name);
            std::fs::write(&path, "data").unwrap();
            path.to_string_lossy().into_owned()
        })
        .to_vec();
    (temp, directory, paths)
}
fn action<'a>(menus: &'a [serde_json::Value], id: &str) -> &'a serde_json::Value {
    menus
        .iter()
        .find(|menu| {
            menu["kind"] == "builtin"
                && menu["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|item| item["id"] == id)
        })
        .unwrap()
}
#[test]
fn router_resolves_builtin_selection_and_execute_uses_its_snapshot() {
    let (_temp, directory, paths) = temp_selection();
    let many = super::query(&directory, &paths).unwrap();
    assert!(many
        .iter()
        .any(|menu| menu["providerId"] == "builtin:batch-rename"));
    assert!(!many
        .iter()
        .any(|menu| menu["providerId"] == "builtin:rename"));
    let menu = action(&many, "batchRename");
    let session = menu["menuSessionId"].as_str().unwrap();
    let result = super::execute(session, "batchRename").unwrap();
    assert_eq!(result["kind"], "coreAction");
    assert_eq!(result["paths"], json!(paths));
    assert!(super::execute(session, "batchRename").is_err());
    let single = super::query(&directory, &paths[..1]).unwrap();
    assert!(single
        .iter()
        .any(|menu| menu["providerId"] == "builtin:rename"));
    assert!(!single
        .iter()
        .any(|menu| menu["providerId"] == "builtin:batch-rename"));
}
#[test]
fn sessions_cannot_invoke_another_provider_or_selection() {
    let (_temp, directory, paths) = temp_selection();
    let first = super::query(&directory, &paths[..1]).unwrap();
    let menu = action(&first, "copy");
    assert!(super::execute(menu["menuSessionId"].as_str().unwrap(), "trash").is_err());
    let second = super::query(&directory, &paths[1..]).unwrap();
    let menu = action(&second, "copy");
    let result = super::execute(menu["menuSessionId"].as_str().unwrap(), "copy").unwrap();
    assert_eq!(result["paths"], json!(&paths[1..]));
}
#[test]
fn expired_sessions_and_disabled_actions_are_rejected() {
    let (_temp, directory, paths) = temp_selection();
    let menus = super::query(&directory, &paths[..1]).unwrap();
    let session_id = action(&menus, "copy")["menuSessionId"]
        .as_str()
        .unwrap()
        .to_string();
    super::sessions()
        .lock()
        .unwrap()
        .get_mut(&session_id)
        .unwrap()
        .created = std::time::Instant::now() - std::time::Duration::from_secs(91);
    assert!(super::execute(&session_id, "copy").is_err());
    let ctx = super::builtin::Context::for_test(&directory, paths, vec![], true, false);
    assert!(super::builtin::execute("builtin:rename", "rename", &ctx).is_err());
}
#[test]
fn trash_group_and_multi_selection_are_provider_owned() {
    let ctx =
        super::builtin::Context::for_test("/trash", vec!["/trash/a".into()], vec![], false, true);
    let result = super::builtin::resolve("builtin:trash", &ctx).unwrap();
    assert_eq!(result["items"][0]["id"], "restore");
    assert_eq!(result["items"][0]["placement"], "tail");
    assert_eq!(result["items"][1]["id"], "delete");
    // The connection cache group only exists for a connection selection.
    assert!(!super::builtin::providers(&ctx)
        .iter()
        .any(|provider| provider.id == "builtin:connection-cache"));
}

fn mount_entry(path: &str, is_dir: bool) -> crate::core::files::Entry {
    crate::core::files::Entry {
        path: path.into(),
        is_dir,
        ..Default::default()
    }
}

fn provider_ids(ctx: &super::builtin::Context) -> Vec<String> {
    super::builtin::providers(ctx)
        .into_iter()
        .map(|provider| provider.id)
        .collect()
}

fn mount_selection(
    path: &str,
    is_dir: bool,
    read_only: bool,
    capabilities: crate::plugins::resource::file_capabilities::FileCapabilities,
) -> super::builtin::Context {
    super::builtin::Context::for_test_caps(
        path.rsplit_once('/').map_or(path, |(parent, _)| parent),
        vec![path.into()],
        vec![mount_entry(path, is_dir)],
        read_only,
        false,
        capabilities,
    )
}

fn emitted_items(ctx: &super::builtin::Context) -> Vec<serde_json::Value> {
    provider_ids(ctx)
        .iter()
        .filter_map(|id| super::builtin::resolve(id, ctx).ok())
        .flat_map(|menu| menu["items"].as_array().cloned().unwrap_or_default())
        .collect()
}

fn assert_never_disabled(ctx: &super::builtin::Context) {
    for item in emitted_items(ctx) {
        assert_eq!(item["disabled"], false, "builtin item was disabled: {item}");
    }
}

#[test]
fn a_read_only_mount_omits_every_mutating_group() {
    let ctx = mount_selection(
        "fiex://mount/gating-mount/report.txt",
        false,
        false,
        crate::plugins::resource::file_capabilities::FileCapabilities::READ_ONLY,
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
    // Unavailable groups are absent, not empty and not disabled.
    for id in ["builtin:rename", "builtin:batch-rename", "builtin:trash"] {
        assert!(super::builtin::resolve(id, &ctx).is_err(), "{id}");
    }
    assert_never_disabled(&ctx);
}

#[test]
fn an_unreadable_file_hides_open_but_keeps_info() {
    let ctx = mount_selection(
        "fiex://mount/gating-mount/report.txt",
        false,
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
        false,
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
    let ctx = mount_selection(
        "fiex://mount/gating-mount/a.txt",
        false,
        false,
        capabilities,
    );
    assert!(provider_ids(&ctx).contains(&"builtin:trash".to_string()));
    let trash = super::builtin::resolve("builtin:trash", &ctx).unwrap();
    // Mounts have no Trash: the provider's `remove` is a permanent delete.
    assert_eq!(trash["items"][0]["id"], "delete");
    assert_eq!(trash["items"][0]["placement"], "tail");
    assert_eq!(trash["items"][0]["danger"], true);
    // Inside the trash both restore and permanent delete are offered.
    let in_trash = super::builtin::Context::for_test_caps(
        "fiex://mount/gating-mount",
        vec!["fiex://mount/gating-mount/a.txt".into()],
        vec![mount_entry("fiex://mount/gating-mount/a.txt", false)],
        false,
        true,
        capabilities,
    );
    let items = super::builtin::resolve("builtin:trash", &in_trash).unwrap()["items"].clone();
    assert_eq!(items[0]["id"], "restore");
    assert_eq!(items[1]["id"], "delete");
    assert_eq!(items[1]["danger"], true);
    assert_never_disabled(&in_trash);
    // A write-only mount keeps the trash group out.
    let without_remove = mount_selection(
        "fiex://mount/gating-mount/a.txt",
        false,
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
fn create_capability_never_adds_a_menu_group() {
    // Folder creation is toolbar-only; the menu must not grow an entry for it.
    let capabilities = crate::plugins::resource::file_capabilities::FileCapabilities {
        read: true,
        write: true,
        create: true,
        rename: true,
        remove: false,
    };
    let ctx = mount_selection(
        "fiex://mount/gating-mount/a.txt",
        false,
        false,
        capabilities,
    );
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
    for id in ["copy", "cut", "duplicate"] {
        assert!(items.iter().any(|item| item["id"] == id), "{id}");
    }
    assert_never_disabled(&ctx);
    // `create` is a toolbar-only capability: the menu is identical without it.
    let without_create = super::builtin::Context::for_test_caps(
        "fiex://mount/gating-mount",
        vec!["fiex://mount/gating-mount/a.txt".into()],
        vec![mount_entry("fiex://mount/gating-mount/a.txt", false)],
        false,
        false,
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
        true,
        false,
        crate::plugins::resource::file_capabilities::FileCapabilities::ALL,
    );
    let ids = provider_ids(&ctx);
    for id in ["builtin:rename", "builtin:batch-rename", "builtin:trash"] {
        assert!(!ids.contains(&id.to_string()), "{id}");
    }
    assert_never_disabled(&ctx);
}

#[test]
fn recent_location_still_offers_open_without_a_filesystem_directory() {
    let (_temp, _, paths) = temp_selection();
    let menus = super::query("recent://", &paths[..1]).unwrap();
    let menu = action(&menus, "open");
    assert_eq!(
        super::execute(menu["menuSessionId"].as_str().unwrap(), "open").unwrap()["path"],
        paths[0]
    );
}

#[test]
fn disabled_actions_are_not_authorized_in_root_or_lazy_menus() {
    let action =
        serde_json::json!({"id":"delete","kind":"action","title":"Delete","disabled":true});
    let response = serde_json::json!({"items":[action]});
    assert!(root_menu_valid(&response).unwrap().is_empty());
    assert!(validated_submenu(&response, "parent", 1)
        .unwrap()
        .actions
        .is_empty());
    let invalid = serde_json::json!({"items":[{"id":"delete","kind":"action","title":"Delete","disabled":"yes"}]});
    assert!(root_menu_valid(&invalid).is_err());
}

#[test]
fn blank_selection_offers_only_capability_gated_create_actions() {
    let capabilities = crate::plugins::resource::file_capabilities::FileCapabilities {
        read: true,
        write: true,
        create: true,
        ..Default::default()
    };
    let ctx = super::builtin::Context::for_test_caps(
        "/tmp/blank",
        vec![],
        vec![],
        false,
        false,
        capabilities,
    );
    assert_eq!(provider_ids(&ctx), ["builtin:create"]);
    let menu = super::builtin::resolve("builtin:create", &ctx).unwrap();
    let items = menu["items"].as_array().unwrap();
    assert_eq!(items[0]["id"], "newFolder");
    assert_eq!(items[1]["id"], "newFile");
    assert_never_disabled(&ctx);

    let files_only = super::builtin::Context::for_test_caps(
        "/tmp/blank",
        vec![],
        vec![],
        false,
        false,
        crate::plugins::resource::file_capabilities::FileCapabilities {
            read: true,
            write: true,
            ..Default::default()
        },
    );
    let only = super::builtin::resolve("builtin:create", &files_only).unwrap();
    let items = only["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], "newFile");

    let empty = super::builtin::Context::for_test_caps(
        "/tmp/blank",
        vec![],
        vec![],
        false,
        false,
        crate::plugins::resource::file_capabilities::FileCapabilities::default(),
    );
    assert!(!provider_ids(&empty).contains(&"builtin:create".to_string()));
}
