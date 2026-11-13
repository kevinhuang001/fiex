use super::super::*;

#[test]
fn resource_identity_is_not_an_operation_grant() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("selected.txt");
    std::fs::write(&file, b"abc").unwrap();
    let resource = crate::plugins::resource::resource_context::new_resource_snapshot(
        file.to_str().unwrap(),
        &file,
    );
    let value = json!(resource.project("plugin", "session", true, false));
    let mut operation = crate::plugins::resource::fs_access::Scope::selected(
        dir.path(),
        std::slice::from_ref(&file),
    )
    .unwrap();
    let other = operation.clone();
    operation.bind_resource(&value, Some(&file)).unwrap();
    assert!(operation.resolve_reference(&value["ref"]).is_ok());
    assert!(other.resolve_reference(&value["ref"]).is_err());
    operation.restrict_to_metadata();
    for method in [
        "host.fs.openRead",
        "host.fs.beginWrite",
        "host.process.run",
        "host.process.run",
        "host.vfs.openView",
    ] {
        assert!(operation.clone().authorize_method(method).is_err());
    }
    assert!(operation.authorize_method("host.fs.stat").is_ok());
    assert!(operation.authorize_method("fs.stat").is_err());
}

#[test]
fn explicit_open_actions_may_create_results_next_to_the_selection() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("selected.zip");
    std::fs::write(&file, b"x").unwrap();
    let file = crate::plugins::resource::fs_access::canonical_selection(&file).unwrap();
    let folder = crate::plugins::resource::fs_access::canonical_selection(dir.path()).unwrap();
    let declared = std::collections::BTreeSet::from([
        "files.create".to_string(),
        "folders.create".to_string(),
    ]);
    assert_eq!(
        action_folders("hooks.invoke", None, std::slice::from_ref(&file), &declared).unwrap(),
        vec![folder.clone()]
    );
    let menu = action_folders(
        "contextMenu.execute",
        Some(dir.path().to_str().unwrap()),
        std::slice::from_ref(&file),
        &declared,
    )
    .unwrap();
    assert!(menu.contains(&folder));
    for (method, declared) in [
        (
            "hooks.invoke",
            std::collections::BTreeSet::from(["files.read".to_string()]),
        ),
        ("preview.open", declared.clone()),
        ("contextMenu.resolve", declared.clone()),
    ] {
        assert!(
            action_folders(method, None, std::slice::from_ref(&file), &declared)
                .unwrap()
                .is_empty()
        );
    }
}
