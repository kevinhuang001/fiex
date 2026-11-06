use super::*;
#[test]
fn native_vfs_uses_the_shared_provider_trait_and_claims_only_registered_paths() {
    assert_eq!(registry().len(), 5);
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
fn path_only_recent_matching_does_not_resolve_persisted_sources() {
    let (view, storage) = crate::plugins::match_rules::typed_locations_for(RECENT_URI);
    assert_eq!(view.unwrap().provider_id, "core:recent");
    assert!(storage.is_none());
    let (_, applications_storage) =
        crate::plugins::match_rules::typed_locations_for(APPLICATIONS_URI);
    assert_eq!(
        applications_storage.unwrap().provider_id,
        "core:applications"
    );
    let (_, trash_storage) = crate::plugins::match_rules::typed_locations_for(trash_vfs::URI);
    assert_eq!(trash_storage.unwrap().provider_id, "core:trash");
}

#[test]
fn native_vfs_registry_binds_domain_owners_without_a_core_module_enum() {
    assert_eq!(
        owner_for_provider("core:recent"),
        Some(CoreOwnerKey::new("recent"))
    );
    assert_eq!(
        owner_for_provider("core:applications"),
        Some(CoreOwnerKey::new("applications"))
    );
    assert_eq!(
        owner_for_provider("core:trash"),
        Some(CoreOwnerKey::new("core"))
    );
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
    assert!(operations(APPLICATIONS_URI).contains(&FsOperation::Write));
    assert!(operations(APPLICATIONS_URI).contains(&FsOperation::Mkdir));
}

#[test]
fn native_vfs_operations_reject_invalid_mutations() {
    assert!(require(RECENT_URI, "core:test", FsOperation::List, false).is_ok());
    assert!(require(RECENT_URI, "core:test", FsOperation::Search, false).is_ok());
    assert!(require(RECENT_URI, "core:test", FsOperation::Write, true).is_err());
    assert!(require("recent://missing", "core:test", FsOperation::Read, false).is_err());
    assert!(require(APPLICATIONS_URI, "core:test", FsOperation::Mkdir, true).is_ok());
    assert!(require(APPLICATIONS_URI, "core:test", FsOperation::Write, true).is_ok());
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
fn recent_navigation_requires_an_existing_resource_with_a_resolvable_parent() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("recent.txt");
    std::fs::write(&file, b"x").unwrap();
    assert_eq!(
        navigable_source(&file.to_string_lossy()),
        Some(file.to_string_lossy().into())
    );
    assert!(navigable_source(&directory.path().to_string_lossy()).is_none());
    assert!(navigable_source(&directory.path().join("missing").to_string_lossy()).is_none());
    assert!(navigable_source("fiex://mount/example/recent.txt").is_none());
}
