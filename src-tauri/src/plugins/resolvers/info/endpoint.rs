//! Native and plugin Info implementations share the SDK endpoint contract.
//! The plugin proxy retains the Core-owned resource snapshot and never
//! accepts its resource identity from plugin-supplied parameters.
use super::native;
use fiex_provider_contracts::{ProviderEndpoint, ProviderRequest};
use crate::plugins::{host::PluginInfo, resource::resource_context::ResourceSnapshot};
use serde_json::{json, Value};
use std::sync::Arc;

pub(super) struct NativeInfoEndpoint {
    pub id: String,
    pub display: String,
    pub resource: Arc<ResourceSnapshot>,
}

impl ProviderEndpoint for NativeInfoEndpoint {
    fn invoke(&self, request: &ProviderRequest) -> Result<Value, String> {
        if request.method != "info.resolve" {
            return Err("unregistered Info event".into());
        }
        let sections = native::invoke(&self.id, &self.display, &self.resource)?;
        Ok(json!({"sections": sections}))
    }
}

pub(super) struct PluginInfoProxy {
    pub plugin: PluginInfo,
    pub resource: Arc<ResourceSnapshot>,
}

impl ProviderEndpoint for PluginInfoProxy {
    fn invoke(&self, request: &ProviderRequest) -> Result<Value, String> {
        if request.method != "info.resolve" {
            return Err("unregistered Info event".into());
        }
        crate::plugins::events::invoke_resources(
            &self.plugin,
            request.method.as_str(),
            request.params.clone(),
            std::slice::from_ref(self.resource.as_ref()),
            None,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_info_contract_rejects_unknown_events() {
        let resource = Arc::new(ResourceSnapshot::from_metadata(
            "/opaque",
            Default::default(),
        ));
        let endpoint = NativeInfoEndpoint {
            id: "core.metadata".into(),
            display: "/opaque".into(),
            resource,
        };
        assert!(endpoint
            .invoke(&ProviderRequest {
                method: "preview.open".into(),
                params: json!({}),
            })
            .is_err());
    }
}
