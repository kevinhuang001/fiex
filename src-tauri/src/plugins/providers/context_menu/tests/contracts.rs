use super::*;

#[test]
fn menu_rejects_unknown_fields_and_unknown_parent() {
    assert!(
        menu_response_valid(&json!({"items": [
            {"id":"root","kind":"submenu","title":"Root"},
            {"id":"open","kind":"action","title":"Open","parentId":"root"}
        ]}))
        .unwrap()
        .contains("open")
    );
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

#[test]
fn disabled_actions_are_not_authorized_in_root_or_lazy_menus() {
    let action =
        serde_json::json!({"id":"delete","kind":"action","title":"Delete","disabled":true});
    let response = serde_json::json!({"items":[action]});
    assert!(root_menu_valid(&response).unwrap().is_empty());
    assert!(
        validated_submenu(&response, "parent", 1)
            .unwrap()
            .actions
            .is_empty()
    );
    let invalid = serde_json::json!({"items":[{"id":"delete","kind":"action","title":"Delete","disabled":"yes"}]});
    assert!(root_menu_valid(&invalid).is_err());
}
