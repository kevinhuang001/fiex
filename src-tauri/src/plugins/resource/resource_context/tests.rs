use super::*;
use std::fs;
#[test]
fn unknown_schemes_fail_before_local_metadata_discovery() {
    for path in ["unknown://not-registered", "unknown:opaque-reference"] {
        assert!(
            crate::plugins::resource::resource_context::resolve(path).is_err(),
            "{path}"
        );
    }
}

#[test]
fn metadata_projection_does_not_grant_read_and_reuses_identity() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("example.txt");
    fs::write(&file, b"abc").unwrap();
    let snapshot = crate::plugins::resource::resource_context::new_resource_snapshot(
        file.to_str().unwrap(),
        &file,
    );
    let without_grant = snapshot.context("org.example", false, false);
    let with_meta = snapshot.context("org.example", true, false);
    assert_eq!(
        without_grant.targets[0].reference,
        with_meta.targets[0].reference
    );
    assert!(without_grant.targets[0].metadata.is_none());
    assert_eq!(
        with_meta.targets[0].metadata.as_ref().unwrap().size,
        Some(3)
    );
}

#[test]
fn projected_context_carries_trusted_view_storage_and_effective_operations() {
    let resource = crate::plugins::resource::resource_context::from_metadata(
        crate::core::volumes::applications::URI,
        ResourceMetadata {
            name: "Applications".into(),
            kind: "directory".into(),
            ..Default::default()
        },
    );
    let projected = resource.project("org.example", "", true, false);
    assert_eq!(
        projected
            .view
            .as_ref()
            .map(|location| location.provider_id.as_str()),
        Some("core:applications")
    );
    assert_eq!(
        projected
            .storage
            .as_ref()
            .map(|location| location.provider_id.as_str()),
        Some("core:applications")
    );
    assert!(
        projected
            .capabilities
            .allows(crate::plugins::provider_registry::FsOperation::Mkdir)
    );
    let wire = serde_json::to_value(&projected).unwrap();
    assert!(wire.get("path").is_none());

    let metadata_hidden = resource.project("org.example", "", false, false);
    assert!(metadata_hidden.metadata.is_none());
    assert!(metadata_hidden.capabilities.operations.is_empty());
    assert!(metadata_hidden.view.is_some());
}
