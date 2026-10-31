//! Trusted FS/VFS view and storage identity resolution for provider matching.
//! These descriptors are derived by Core from its registries, never from
//! plugin-supplied metadata. A failed mount/source lookup is not an FS fallback.
use serde_json::{json, Value};

fn descriptor(kind: &str, provider_id: &str, mount_id: &str) -> Value {
    json!({"type": kind, "providerId": provider_id, "mountId": mount_id})
}

fn storage_location(path: &str) -> Option<Value> {
    if let Some(view) = crate::core::volumes::virtual_resolvers::view_for(path) {
        if view.id() == "core:recent" {
            return None;
        }
        return Some(descriptor("vfs", view.id(), view.id()));
    }
    if crate::usbfs::route(path).is_some() {
        // USB drives expose native FS operations, but have their own owner.
        let mount = crate::plugins::resource::resource_context::ResourceSnapshot::from_metadata(
            path,
            crate::plugins::contracts::ResourceMetadata::default(),
        )
        .reference
        .mount_id;
        return Some(descriptor("fs", "core:usb-fs", &mount));
    }
    if crate::plugins::mounts::handles(path) {
        let (uri, source) = crate::plugins::mounts::resolve(path).ok()?;
        // Package ID, contribution type and concrete mount are separate
        // identities. Core establishes the mapping, not the plugin process.
        let provider = match source {
            crate::plugins::mounts::Source::Connection(conn) => {
                format!("plugin:{}:{}", conn.plugin_id, conn.kind)
            }
            crate::plugins::mounts::Source::View(view) => format!(
                "plugin:{}:{}",
                view.descriptor.plugin_id, view.descriptor.contribution_id
            ),
        };
        return Some(descriptor("vfs", &provider, &uri.mount_id));
    }
    // An unrecognized URL is not a native path. Do not promote unavailable
    // or revoked VFS resources to core:local-fs.
    if path.is_empty() || path.contains("://") {
        return None;
    }
    Some(descriptor("fs", "core:local-fs", "local"))
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
