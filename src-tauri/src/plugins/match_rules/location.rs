//! Trusted FS/VFS view and storage identity resolution for provider matching.
//! These descriptors are derived by Core from its registries, never from
//! plugin-supplied metadata. A failed mount/source lookup is not an FS fallback.
use serde_json::{json, Value};

fn descriptor(kind: &str, provider_id: &str, mount_id: &str) -> Value {
    json!({"type": kind, "providerId": provider_id, "mountId": mount_id})
}

fn storage_location(path: &str) -> Option<Value> {
    let (kind, provider, mount) = crate::core::fs::provider_registry::identity(path)?;
    Some(descriptor(kind, &provider, &mount))
}

/// The view determines presentation contributions; storage controls actual IO.
/// Recent aliases retain their view identity but resolve storage only while
/// their source remains available and authorized.
pub fn locations_for(path: &str) -> (Option<Value>, Option<Value>) {
    if let Some(view) = crate::core::volumes::virtual_resolvers::view_for(path) {
        let current = Some(descriptor("vfs", view.id(), view.id()));
        if view.id() == "core:recent" {
            let source = crate::core::recent::source_for(path).ok();
            let storage = source.as_deref().and_then(storage_location);
            return (current, storage);
        }
        return (current.clone(), current);
    }
    let storage = storage_location(path);
    (storage.clone(), storage)
}

/// A blank-area menu has no selected target, but its directory is a target
/// for location, kind and capability matching. All fields come from Core.
pub fn directory_metadata(directory: &str) -> Option<Value> {
    let (view, storage) = locations_for(directory);
    let view = view?;
    let allowed = crate::plugins::resource::file_capabilities::for_path(directory);
    let operations = crate::plugins::provider_registry::FsOperation::ALL
        .into_iter()
        .filter(|op| allowed.allows(*op))
        .map(|op| op.as_str())
        .collect::<Vec<_>>();
    Some(json!({
        "kind": "directory", "extension": "", "mime": "",
        "operations": operations, "view": view, "storage": storage
    }))
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
    fn unknown_mount_never_becomes_local_storage() {
        let (view, storage) =
            locations_for("fiex://mount/00000000000000000000000000000000/resource");
        assert!(view.is_none() && storage.is_none());
        let (view, storage) = locations_for("unrecognized://something");
        assert!(view.is_none() && storage.is_none());
    }
}
