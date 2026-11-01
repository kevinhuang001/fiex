//! Trusted FS/VFS view and storage identity resolution for provider matching.
//! These descriptors are derived by Core from its registries, never from
//! plugin-supplied metadata. A failed mount/source lookup is not an FS fallback.
use fiex_provider_contracts::{
    FsOperation, MatchResource, ResourceCapabilities, ResourceLocation, ResourceMetadata,
    ResourceType,
};
use serde_json::Value;

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

/// Recent storage matching must not advertise an invalid or revoked source.
/// Resolve the owning registered mount once; only local paths require an OS
/// existence probe. This never tries remote IO to guess source identity.
fn projected_storage_location(source: &str) -> Option<ResourceLocation> {
    let (kind, provider, mount) = crate::core::fs::provider_registry::identity(source)?;
    if provider == "core:local-fs" && std::fs::symlink_metadata(source).is_err() {
        return None;
    }
    if provider == "core:usb-fs" && mount == "core:usb:offline" {
        return None;
    }
    descriptor(kind, &provider, &mount)
}

/// The view determines presentation contributions; storage controls actual IO.
/// Recent aliases retain their view identity but resolve storage only while
/// their source remains available and authorized.
pub fn typed_locations_for(path: &str) -> (Option<ResourceLocation>, Option<ResourceLocation>) {
    if let Some(view) = crate::core::volumes::virtual_resolvers::view_for(path) {
        let current = descriptor("vfs", view.id(), view.id());
        let storage = match view.storage_projection(path) {
            crate::core::volumes::virtual_resolvers::NativeStorageProjection::Owned => {
                current.clone()
            }
            crate::core::volumes::virtual_resolvers::NativeStorageProjection::Projected(source) => {
                source.as_deref().and_then(projected_storage_location)
            }
        };
        return (current, storage);
    }
    let storage = storage_location(path);
    (storage.clone(), storage)
}

/// JSON projection used only at the plugin protocol boundary. Matching and
/// authorization consume `typed_locations_for` directly.
pub fn locations_for(path: &str) -> (Option<Value>, Option<Value>) {
    let (view, storage) = typed_locations_for(path);
    (
        view.and_then(|location| serde_json::to_value(location).ok()),
        storage.and_then(|location| serde_json::to_value(location).ok()),
    )
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
                .filter(|op| allowed.allows(*op))
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
            let (_, storage) = locations_for(path);
            let storage = storage.unwrap();
            assert_eq!(storage["type"], kind);
            assert_eq!(storage["providerId"], provider);
            assert_eq!(storage["mountId"], mount);
        }
    }

    #[test]
    fn trusted_view_owner_is_not_inferred_from_plugin_payloads() {
        let (view, storage) = locations_for("applications://");
        assert_eq!(view.as_ref().unwrap()["providerId"], "core:applications");
        assert_eq!(storage.as_ref().unwrap()["type"], "vfs");
        let (view, storage) = locations_for("/tmp/fiex-location-test");
        assert_eq!(view.as_ref().unwrap()["providerId"], "core:local-fs");
        assert_eq!(storage.as_ref().unwrap()["type"], "fs");
    }

    #[test]
    fn stale_recent_local_source_never_advertises_storage_owner() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("recent.txt");
        assert!(projected_storage_location(&path.to_string_lossy()).is_none());
        std::fs::write(&path, b"data").unwrap();
        let live = projected_storage_location(&path.to_string_lossy()).unwrap();
        assert_eq!(live["type"], "fs");
        assert_eq!(live["providerId"], "core:local-fs");
        assert!(projected_storage_location("unregistered://file").is_none());
    }

    #[test]
    fn unknown_mount_never_becomes_local_storage() {
        let (view, storage) =
            locations_for("fiex://mount/00000000000000000000000000000000/resource");
        assert!(view.is_none() && storage.is_none());
        let (view, storage) = locations_for("unrecognized://something");
        assert!(view.is_none() && storage.is_none());
    }
}
