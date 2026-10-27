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
    let ctx = super::builtin::Context {
        directory,
        paths,
        entries: vec![],
        read_only: true,
        in_trash: false,
    };
    assert!(super::builtin::execute("builtin:rename", "rename", &ctx).is_err());
}
#[test]
fn trash_group_and_multi_selection_are_provider_owned() {
    let ctx = super::builtin::Context {
        directory: "/trash".into(),
        paths: vec!["/trash/a".into()],
        entries: vec![],
        read_only: false,
        in_trash: true,
    };
    let result = super::builtin::resolve("builtin:trash", &ctx).unwrap();
    assert_eq!(result["items"][0]["id"], "restore");
    assert_eq!(result["items"][0]["placement"], "tail");
    assert_eq!(result["items"][1]["id"], "delete");
    assert!(
        super::builtin::resolve("builtin:connection-cache", &ctx).unwrap()["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
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
