//! Search plugin transport. Core supplies only a bound ResourceRef and a
//! scope-narrowed request; no plugin-provided path can select its own owner.
use fiex_provider_contracts::{ProviderEndpoint, ProviderRequest};
use crate::plugins::{host::PluginInfo, resource::resource_context::ResourceSnapshot};
use serde_json::Value;

pub(super) struct PluginSearchEndpoint<'a> {
    pub plugin: PluginInfo,
    pub resources: &'a [ResourceSnapshot],
    pub directory: &'a str,
}

impl ProviderEndpoint for PluginSearchEndpoint<'_> {
    fn invoke(&self, request: &ProviderRequest) -> Result<Value, String> {
        if request.method != "search.query" {
            return Err("unsupported SearchProvider event".into());
        }
        crate::plugins::events::invoke_resources(
            &self.plugin,
            &request.method,
            request.params.clone(),
            self.resources,
            Some(self.directory),
        )
    }
}
