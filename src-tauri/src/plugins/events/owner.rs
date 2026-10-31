//! Single-owner plugin event invocation uses the same endpoint as all other
//! Provider families. Scope projection and output validation stay Core-owned.
use crate::plugins::{host::PluginInfo, resource::resource_context::ResourceSnapshot};
use fiex_provider_contracts::{ProviderEndpoint, ProviderRequest};
use serde_json::Value;

pub(super) struct PluginOwner<'a> {
    pub plugin: PluginInfo,
    pub resources: &'a [ResourceSnapshot],
    pub directory: Option<&'a str>,
}

impl ProviderEndpoint for PluginOwner<'_> {
    fn invoke(&self, request: &ProviderRequest) -> Result<Value, String> {
        super::invoke_resources(
            &self.plugin,
            &request.method,
            request.params.clone(),
            self.resources,
            self.directory,
        )
    }
}
