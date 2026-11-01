use super::*;
#[test]
fn native_vfs_uses_the_shared_provider_trait_and_claims_only_registered_paths() {
    assert_eq!(registry().len(), 3);
    for descriptor in registry().descriptors() {
        assert_eq!(descriptor.kind, ProviderKind::Vfs);
        descriptor.validate().unwrap();
    }
    assert_eq!(provider_id("recent://item"), Some("core:recent"));
    assert_eq!(
        provider_id("applications://item"),
        Some("core:applications")
    );
    assert_eq!(provider_id(trash_vfs::URI), Some("core:trash"));
    assert!(provider_id("unknown://item").is_none());
    assert!(provider_id("fiex://mount/not-a-native-view").is_none());
    assert!(!opaque("unknown://item"));
    assert!(resource_entry(RECENT_URI).is_ok());
    assert!(list(RECENT_URI, "core:test").is_ok());
    assert!(list("recent://bad", "core:test").is_err());
}

#[test]
fn native_view_storage_projection_is_preserved() {
    assert!(matches!(
        storage_projection(RECENT_URI),
        NativeStorageProjection::Projected(None)
    ));
    assert!(matches!(
        storage_projection(APPLICATIONS_URI),
        NativeStorageProjection::Owned
    ));
    assert!(matches!(
        storage_projection(trash_vfs::URI),
        NativeStorageProjection::Owned
    ));
}

#[test]
fn native_vfs_operation_discovery_is_typed_and_does_not_overclaim() {
    let roots = [RECENT_URI, APPLICATIONS_URI, recycle::URI];
    for root in roots {
        let declared = operations(root);
        let entry = resource_entry(root).unwrap();
        let from_entry = entry
            .allowed_ops
            .iter()
            .filter_map(|value| FsOperation::parse(value))
            .collect::<Vec<_>>();
        assert_eq!(declared, from_entry, "operation DTO mismatch: {root}");
        for operation in FsOperation::ALL {
            assert_eq!(
                require(root, "core:test", operation, false).is_ok(),
                declared.contains(&operation),
                "root {root} overclaims or loses {}",
                operation.as_str()
            );
        }
    }
    assert!(!operations(APPLICATIONS_URI).contains(&FsOperation::Write));
}

#[test]
fn native_vfs_operations_reject_invalid_mutations() {
    assert!(require(RECENT_URI, "core:test", FsOperation::List, false).is_ok());
    assert!(require(RECENT_URI, "core:test", FsOperation::Search, false).is_ok());
    assert!(require(RECENT_URI, "core:test", FsOperation::Write, true).is_err());
    assert!(require("recent://missing", "core:test", FsOperation::Read, false).is_err());
    assert!(require(APPLICATIONS_URI, "core:test", FsOperation::Mkdir, true).is_err());
    assert!(require(APPLICATIONS_URI, "core:test", FsOperation::Remove, false).is_err());
    assert!(require(APPLICATIONS_URI, "core:test", FsOperation::Read, false).is_err());
    assert!(require(
        "trash://not-a-valid-node",
        "core:test",
        FsOperation::Remove,
        false
    )
    .is_err());
    assert!(require(
        "trash://not-a-valid-node",
        "core:test",
        FsOperation::Write,
        true
    )
    .is_err());
}

#[test]
fn recent_reveal_only_accepts_existing_local_files() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("recent.txt");
    std::fs::write(&file, b"x").unwrap();
    assert!(revealable_source(&file.to_string_lossy()));
    assert!(!revealable_source(&directory.path().to_string_lossy()));
    assert!(!revealable_source("fiex://mount/example/recent.txt"));
    assert!(!revealable_source("recent://invalid"));
}
