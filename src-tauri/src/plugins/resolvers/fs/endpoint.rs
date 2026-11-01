//! Per-mount VFS operation gate. The shared VfsProvider proxy owns dispatch;
//! this module only checks negotiated capabilities at the RPC boundary.
use super::Mount;

/// Operation identity is bound to this Core-owned mount, not just the plugin
/// package's union of VFS capabilities. Unknown methods fail closed.
pub(super) fn method_allowed(mount: &Mount, method: &str) -> bool {
    if let Some(operation) = fiex_provider_contracts::FsOperation::from_method(method) {
        return crate::plugins::provider_registry::supports_mount(
            &mount.plugin_id,
            &mount.mount_id,
            operation,
        );
    }
    method == "vfs.discover"
        || (method == "vfs.preflight"
            && crate::plugins::provider_registry::preflight_mount(
                &mount.plugin_id,
                &mount.mount_id,
            ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::Source;
    use serde_json::json;

    #[test]
    fn endpoint_rejects_foreign_methods_before_opening_plugin_host() {
        let mount = Mount {
            mount_id: "m".into(),
            plugin_id: "org.invalid".into(),
            source: Source::View {
                contribution_id: "view".into(),
                view_context: json!({}),
            },
            scope: None,
        };
        assert!(!method_allowed(&mount, "contextMenu.execute"));
        for method in ["vfs.notImplemented", "vfs.list", "vfs.preflight"] {
            assert!(!method_allowed(&mount, method));
        }
    }
}
