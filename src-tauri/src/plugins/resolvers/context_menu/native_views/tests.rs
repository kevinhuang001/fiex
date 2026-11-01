use super::*;
use crate::core::volumes::{applications, recycle, trash_vfs};
use fiex_provider_contracts::ProviderKind;

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
        execute("core.applications.menu", "trash", "applications://", &user).unwrap()["command"],
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
        let owner = match descriptor.id.as_str() {
            "core.recent.menu" => "core:recent",
            "core.applications.menu" => "core:applications",
            "core.trash.menu" => "core:trash",
            other => panic!("unexpected native menu provider: {other}"),
        };
        let matching = MatchResource {
            view: Some(fiex_provider_contracts::ResourceLocation {
                resource_type: fiex_provider_contracts::ResourceType::Vfs,
                provider_id: owner.into(),
                mount_id: Some(owner.into()),
            }),
            ..Default::default()
        };
        assert!(descriptor.r#match.matches_targets(&[matching], 1));
        let (view, storage) = crate::plugins::match_rules::typed_locations_for("/tmp");
        let foreign = MatchResource {
            view,
            storage,
            ..Default::default()
        };
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
    assert_eq!(launched["command"], "open");
    assert_eq!(launched["paths"], json!(system_app));
}

#[test]
fn blank_trash_canvas_offers_empty_without_selection() {
    let blank = Vec::<String>::new();
    for directory in [trash_vfs::URI, recycle::URI] {
        let candidates = providers(directory, &blank);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].id, "core.trash.menu");
        let result = resolve("core.trash.menu", directory, &blank).unwrap();
        let items = result["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], "emptyTrash");
        assert_eq!(items[0]["danger"], true);
        assert_eq!(
            execute("core.trash.menu", "emptyTrash", directory, &blank).unwrap()["command"],
            "emptyTrash"
        );
        assert!(execute("core.trash.menu", "delete", directory, &blank).is_err());
    }
    assert!(providers(applications::URI, &blank).is_empty());
    assert!(providers("recent://", &blank).is_empty());
    assert!(providers("/tmp", &blank).is_empty());
}

#[test]
fn trash_selection_deletes_exactly_the_offered_nodes() {
    let paths = vec!["trash://Zmlyc3Q".into(), "trash://c2Vjb25k".into()];
    let single = resolve("core.trash.menu", trash_vfs::URI, &paths[..1]).unwrap();
    let items = single["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["id"], "restore");
    assert_eq!(items[1]["id"], "delete");

    let multiple = resolve("core.trash.menu", trash_vfs::URI, &paths).unwrap();
    let items = multiple["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], "delete");
    let effect = execute("core.trash.menu", "delete", trash_vfs::URI, &paths).unwrap();
    assert_eq!(effect["command"], "delete");
    assert_eq!(effect["paths"], json!(paths));
    assert!(execute("core.trash.menu", "restore", trash_vfs::URI, &paths).is_err());
    assert!(execute("core.trash.menu", "emptyTrash", trash_vfs::URI, &paths).is_err());
    assert!(providers(trash_vfs::URI, &["/tmp/not-in-trash".into()]).is_empty());
}
