//! IPC implementation of the same ProviderEndpoint contract as native
//! providers. Mount identity and scope are Core-owned and added here, never
//! accepted from the caller's request parameters.
use super::{Mount, Source};
use fiex_provider_contracts::{ProviderEndpoint, ProviderRequest};
use serde_json::{json, Value};

pub(super) struct PluginFsEndpoint<'a> {
    pub mount: &'a Mount,
}

/// Operation identity is bound to this Core-owned mount, not just the plugin
/// package's union of VFS capabilities. Unknown methods fail closed.
fn method_allowed(mount: &Mount, method: &str) -> bool {
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

impl ProviderEndpoint for PluginFsEndpoint<'_> {
    fn invoke(&self, request: &ProviderRequest) -> Result<Value, String> {
        if !method_allowed(self.mount, &request.method) {
            return Err("unsupported or unregistered VFS operation for this mount".into());
        }
        let mut params = request.params.clone();
        let object = params
            .as_object_mut()
            .ok_or("file-system parameters must be an object")?;
        // A plugin does not choose its mount, connection credentials or the
        // standalone view's contribution. Always overwrite untrusted fields.
        object.insert("mountId".into(), json!(self.mount.mount_id));
        match &self.mount.source {
            Source::Connection(config) => {
                object.remove("contributionId");
                object.remove("viewContext");
                object.insert("connection".into(), config.clone());
            }
            Source::View {
                contribution_id,
                view_context,
            } => {
                object.remove("connection");
                object.insert("contributionId".into(), json!(contribution_id));
                object.insert("viewContext".into(), view_context.clone());
            }
        }
        crate::plugins::host::global().invoke_fs(
            &self.mount.plugin_id,
            &request.method,
            params,
            self.mount.scope.clone(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let endpoint = PluginFsEndpoint { mount: &mount };
        let result = endpoint.invoke(&ProviderRequest {
            method: "contextMenu.execute".into(),
            params: json!({}),
        });
        assert!(result.is_err());
        for method in ["vfs.notImplemented", "vfs.list", "vfs.preflight"] {
            assert!(!method_allowed(&mount, method));
            assert!(endpoint
                .invoke(&ProviderRequest {
                    method: method.into(),
                    params: json!({}),
                })
                .is_err());
        }
    }
}
