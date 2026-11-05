//! Trusted FS/VFS view and storage identity resolution for provider matching.
//! These descriptors are derived by Core from its registries, never from
//! plugin-supplied metadata. A failed mount/source lookup is not an FS fallback.
use fiex_provider_contracts::{
    FsOperation, MatchResource, ResourceCapabilities, ResourceLocation, ResourceMetadata,
    ResourceType,
};

fn descriptor(kind: &str, provider_id: &str, mount_id: &str) -> Option<ResourceLocation> {
    let resource_type = match kind {
        "fs" => ResourceType::Fs,
        "vfs" => ResourceType::Vfs,
        _ => return None,
    };
    Some(ResourceLocation {
        resource_type,
        provider_id: provider_id.into(),
        mount_id: Some(mount_id.into()),
    })
}

fn storage_location(path: &str) -> Option<ResourceLocation> {
    let (kind, provider, mount) = crate::core::fs::provider_registry::identity(path)?;
    descriptor(kind, &provider, &mount)
}

fn projected_storage_location(source: &str) -> Option<ResourceLocation> {
    let identity = crate::core::volumes::native_vfs::storage_identity(source)?;
    if !identity.available {
        return None;
    }
    if identity.host_path && std::fs::symlink_metadata(source).is_err() {
        return None;
    }
    let kind = match identity.storage_type? {
        ResourceType::Fs => "fs",
        ResourceType::Vfs => "vfs",
    };
    descriptor(kind, identity.provider_id, &identity.mount_id)
}

pub(crate) fn projected_storage_for_source(source: &str) -> Option<ResourceLocation> {
    projected_storage_location(source)
}

/// The view determines presentation contributions; storage controls actual IO.
/// Recent aliases retain their view identity but resolve storage only while
/// their source remains available and authorized.
pub fn typed_locations_for(path: &str) -> (Option<ResourceLocation>, Option<ResourceLocation>) {
    if let Some(provider_id) = crate::core::volumes::native_vfs::provider_id(path) {
        let current = descriptor("vfs", provider_id, provider_id);
        let storage = storage_location(path);
        return (current, storage);
    }
    let storage = storage_location(path);
    (storage.clone(), storage)
}

pub(crate) fn typed_locations_for_snapshot(
    resource: &crate::plugins::resource::resource_context::ResourceSnapshot,
) -> (Option<ResourceLocation>, Option<ResourceLocation>) {
    let (view, own_storage) = typed_locations_for(&resource.display);
    let storage = match &resource.storage_projection {
        fiex_resource::StorageProjection::Owned => own_storage,
        fiex_resource::StorageProjection::Projected(source) => source.clone(),
    };
    (view, storage)
}

/// Typed Core snapshot for blank-canvas matching. No host path or plugin
/// supplied location can enter this matcher input.
pub fn directory_match_resource(directory: &str) -> Option<MatchResource> {
    let (view, storage) = typed_locations_for(directory);
    view.as_ref()?;
    let allowed = crate::plugins::resource::file_capabilities::for_path(directory);
    Some(MatchResource {
        metadata: Some(ResourceMetadata {
            kind: "directory".into(),
            extension: Some(String::new()),
            mime: Some(String::new()),
            ..Default::default()
        }),
        view,
        storage,
        capabilities: ResourceCapabilities {
            operations: FsOperation::ALL
                .into_iter()
                .filter(|operation| allowed.allows(*operation))
                .collect(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_operation_and_matching_use_same_trusted_storage_identity() {
        for path in ["/tmp/fiex-storage", "applications://"] {
            let (kind, provider, mount) =
                crate::core::fs::provider_registry::identity(path).unwrap();
            let (_, storage) = typed_locations_for(path);
            let storage = storage.unwrap();
            assert_eq!(
                storage.resource_type,
                match kind {
                    "fs" => ResourceType::Fs,
                    "vfs" => ResourceType::Vfs,
                    _ => unreachable!(),
                }
            );
            assert_eq!(storage.provider_id, provider);
            assert_eq!(storage.mount_id.as_deref(), Some(mount.as_str()));
        }
    }

    #[test]
    fn trusted_view_owner_is_not_inferred_from_plugin_payloads() {
        let (view, storage) = typed_locations_for("applications://");
        assert_eq!(view.as_ref().unwrap().provider_id, "core:applications");
        assert_eq!(storage.as_ref().unwrap().resource_type, ResourceType::Vfs);
    }

    #[test]
    fn stale_recent_local_source_never_advertises_storage_owner() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("recent.txt");
        assert!(projected_storage_location(&path.to_string_lossy()).is_none());
        std::fs::write(&path, b"data").unwrap();
        let live = projected_storage_location(&path.to_string_lossy()).unwrap();
        assert_eq!(live.resource_type, ResourceType::Fs);
        assert_eq!(live.provider_id, "core:local-fs");
        assert!(projected_storage_location("unregistered://file").is_none());
    }

    #[test]
    fn recent_matching_uses_core_projection_captured_with_the_entry() {
        let projected = descriptor("fs", "core:local-fs", "local").unwrap();
        let entry = crate::core::files::Entry {
            path: "recent://entry".into(),
            storage_projection: fiex_resource::StorageProjection::Projected(Some(
                projected.clone(),
            )),
            ..Default::default()
        };
        let resource = crate::plugins::resource::resource_context::from_entry(&entry.path, &entry);
        let (view, storage) = typed_locations_for_snapshot(&resource);
        assert_eq!(view.unwrap().provider_id, "core:recent");
        assert_eq!(storage, Some(projected));
        assert!(typed_locations_for("recent://entry").1.is_none());
    }

    #[test]
    fn unknown_mount_never_becomes_local_storage() {
        let (view, storage) =
            typed_locations_for("fiex://mount/00000000000000000000000000000000/resource");
        assert!(view.is_none() && storage.is_none());
        let (view, storage) = typed_locations_for("unrecognized://something");
        assert!(view.is_none() && storage.is_none());
    }
}
