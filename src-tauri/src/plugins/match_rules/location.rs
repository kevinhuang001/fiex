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

/// Recent storage matching must not advertise an invalid or revoked source.
/// Resolve the owning registered mount once; only local paths require an OS
/// existence probe. This never tries remote IO to guess source identity.
fn recent_storage_location(source: &str) -> Option<Value> {
    let (kind, provider, mount) = crate::core::fs::provider_registry::identity(source)?;
    if provider == "core:local-fs" && std::fs::symlink_metadata(source).is_err() {
        return None;
    }
    if provider == "core:usb-fs" && mount == "core:usb:offline" {
        return None;
    }
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
            let storage = source.as_deref().and_then(recent_storage_location);
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
    fn stale_recent_local_source_never_advertises_storage_owner() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("recent.txt");
        assert!(recent_storage_location(&path.to_string_lossy()).is_none());
        std::fs::write(&path, b"data").unwrap();
        let live = recent_storage_location(&path.to_string_lossy()).unwrap();
        assert_eq!(live["type"], "fs");
        assert_eq!(live["providerId"], "core:local-fs");
        assert!(recent_storage_location("unregistered://file").is_none());
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
