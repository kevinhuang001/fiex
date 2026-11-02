use super::*;

#[test]
fn registered_connections_use_exact_shared_provider_descriptors() {
    assert_eq!(registry_len(), 4);
    for descriptor in descriptors() {
        assert_eq!(descriptor.kind, ProviderKind::ConnectionContextMenu);
        descriptor.validate().unwrap();
    }
    let target = MatchResource {
        metadata: Some(fiex_provider_contracts::ResourceMetadata {
            kind: "connection".into(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let candidates = providers(&[target]);
    assert_eq!(candidates.len(), 4);
    assert_eq!(candidates[0].id, "builtin:connection-edit");
    assert_eq!(candidates[3].id, "builtin:connection-close");
    assert!(get("builtin:unregistered").is_none());
}
