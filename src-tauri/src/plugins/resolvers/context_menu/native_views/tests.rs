use super::*;
use crate::core::volumes::applications;

#[test]
fn view_descriptors_are_disjoint_and_not_plugin_privileged() {
    let applications = vec!["applications://c3lzdGVtLWFwcA".into()];
    let recent = vec!["recent://not-saved".into()];
    let root = vec!["/tmp/example".into()];
    assert_eq!(
        providers("applications://", &applications)[0].id,
        "core.applications.menu"
    );
    assert_eq!(providers("recent://", &recent)[0].id, "core.recent.menu");
    assert!(providers("/tmp", &root).is_empty());
    assert!(providers(
        "applications://",
        &[applications[0].clone(), recent[0].clone()]
    )
    .is_empty());
}

#[test]
fn system_app_is_launch_only_and_user_entries_are_protected() {
    let system = vec!["applications://c3lzdGVtLWFwcA".into()];
    let value = resolve("core.applications.menu", "applications://", &system).unwrap();
    assert_eq!(value["items"].as_array().unwrap().len(), 1);
    assert_eq!(value["items"][0]["id"], "open");
    assert!(execute(
        "core.applications.menu",
        "rename",
        "applications://",
        &system
    )
    .is_err());
    let user = vec![format!("{}user/dXNlci1hcHA", applications::URI)];
    let value = resolve("core.applications.menu", "applications://", &user).unwrap();
    assert_eq!(value["items"].as_array().unwrap().len(), 3);
    assert_eq!(
        execute("core.applications.menu", "trash", "applications://", &user)
            .unwrap()
            .command,
        "trashApplications"
    );
}

#[test]
fn mixed_app_selections_have_only_shared_actions() {
    let selections = vec![
        "applications://c3lzdGVtLWFwcA".into(),
        format!("{}user/dXNlci1hcHA", applications::URI),
    ];
    let value = resolve("core.applications.menu", "applications://", &selections).unwrap();
    assert_eq!(value["items"].as_array().unwrap().len(), 1);
    assert_eq!(value["items"][0]["id"], "open");
}

#[test]
fn native_view_menus_use_validated_shared_descriptors() {
    let registered = registry();
    assert_eq!(registered.len(), 3);
    for descriptor in registered.descriptors() {
        assert_eq!(descriptor.kind, ProviderKind::ContextMenu);
        descriptor.validate().unwrap();
        let owner = registered.get(&descriptor.id).unwrap().owner();
        let matching = json!({
            "view": {"type":"vfs","providerId":owner,"mountId":owner}
        });
        assert!(descriptor.r#match.matches_targets(&[matching], 1));
        let foreign = json!({
            "view": {"type":"fs","providerId":"core:local-fs","mountId":"local"}
        });
        assert!(!descriptor.r#match.matches_targets(&[foreign], 1));
    }
}

#[test]
fn native_menu_executes_only_actions_offered_for_this_exact_selection() {
    let system_app = vec!["applications://c3lzdGVtLWFwcA".into()];
    assert!(execute(
        "core.applications.menu",
        "trash",
        "applications://",
        &system_app
    )
    .is_err());
    assert!(execute(
        "core.applications.menu",
        "delete",
        "applications://",
        &system_app
    )
    .is_err());
    let launched = execute(
        "core.applications.menu",
        "open",
        "applications://",
        &system_app,
    )
    .unwrap();
    assert_eq!(launched.command, "open");
    assert_eq!(launched.paths, system_app);
}
