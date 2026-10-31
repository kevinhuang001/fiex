use super::*;

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

