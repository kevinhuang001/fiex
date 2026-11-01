//! Native and plugin Info implementations share the SDK endpoint contract.
//! The plugin proxy retains the Core-owned resource snapshot and never
//! accepts its resource identity from plugin-supplied parameters.
use super::{native, proxy::ScopedInfoProxy};
use crate::plugins::resource::resource_context::ResourceSnapshot;
use fiex_provider_contracts::{ProviderEndpoint, ProviderRequest};
use serde_json::{json, Value};
use std::sync::Arc;

pub(super) struct NativeInfoEndpoint {
    pub id: String,
    pub resource: Arc<ResourceSnapshot>,
}

impl ProviderEndpoint for NativeInfoEndpoint {
    fn invoke(&self, request: &ProviderRequest) -> Result<Value, String> {
        if request.method != "info.resolve" {
            return Err("unregistered Info event".into());
        }
        let sections = native::invoke(&self.id, &self.resource).map_err(|error| error.to_string())?;
        Ok(json!({"sections": sections}))
    }
}

impl ProviderEndpoint for ScopedInfoProxy {
    fn invoke(&self, request: &ProviderRequest) -> Result<Value, String> {
        let sections = self
            .resolve_typed(request)
            .map_err(|error| error.to_string())?;
        Ok(json!({"sections": sections}))
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
