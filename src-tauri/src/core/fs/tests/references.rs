use super::*;

#[test]
fn resource_identity_is_stable_across_fresh_snapshots() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a");
    let b = dir.path().join("b");
    std::fs::write(&a, b"a").unwrap();
    std::fs::write(&b, b"b").unwrap();
    assert_eq!(
        crate::plugins::resource::resource_context::new_resource_snapshot(a.to_str().unwrap(), &a)
            .reference,
        crate::plugins::resource::resource_context::resolve(a.to_str().unwrap())
            .unwrap()
            .reference
    );
    assert_ne!(
        crate::plugins::resource::resource_context::new_resource_snapshot(a.to_str().unwrap(), &a)
            .reference,
        crate::plugins::resource::resource_context::new_resource_snapshot(b.to_str().unwrap(), &b)
            .reference
    );
}

#[test]
fn revoking_a_derived_resource_invalidates_its_children_in_every_scope_clone() {
    use crate::plugins::contracts::ResourceMetadata;

    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("directory");
    std::fs::create_dir(&directory).unwrap();
    let nested = directory.join("nested.txt");
    std::fs::write(&nested, b"data").unwrap();

    let mut scope = Scope::selected(root.path(), &[root.path().to_path_buf()]).unwrap();
    let root_snapshot = crate::plugins::resource::resource_context::new_resource_snapshot(
        root.path().to_str().unwrap(),
        root.path(),
    );
    let root_target = json!({"ref":root_snapshot.reference,"metadata":root_snapshot.metadata()});
    scope
        .bind_resource(&root_target, Some(root.path()))
        .unwrap();

    let directory_ref = scope
        .issue_child(
            &root_target["ref"],
            directory.to_str().unwrap(),
            Some(ResourceMetadata {
                name: "directory".into(),
                kind: "directory".into(),
                ..Default::default()
            }),
        )
        .unwrap();
    let file_ref = scope
        .issue_child(
            &directory_ref["ref"],
            nested.to_str().unwrap(),
            Some(ResourceMetadata {
                name: "nested.txt".into(),
                kind: "file".into(),
                ..Default::default()
            }),
        )
        .unwrap();
    let task_scope = scope.clone();

    assert!(scope.resource_bound(&directory_ref["ref"]).unwrap());
    assert!(task_scope.resource_bound(&file_ref["ref"]).unwrap());
    assert!(
        scope
            .revoke_issued_reference(&directory_ref["ref"])
            .unwrap()
    );
    assert!(!task_scope.resource_bound(&directory_ref["ref"]).unwrap());
    assert!(!task_scope.resource_bound(&file_ref["ref"]).unwrap());
    assert!(task_scope.resolve_reference(&file_ref["ref"]).is_err());
    assert!(
        task_scope
            .issue_child(
                &root_target["ref"],
                directory.to_str().unwrap(),
                Some(ResourceMetadata {
                    name: "directory".into(),
                    kind: "directory".into(),
                    ..Default::default()
                }),
            )
            .is_err()
    );
    assert!(task_scope.resolve_reference(&root_target["ref"]).is_ok());
}
