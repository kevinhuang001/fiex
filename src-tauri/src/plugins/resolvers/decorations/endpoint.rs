//! Unified decoration endpoint for Core-owned decorations and plugin IPC.
use super::types::DecorationEntry;
use crate::plugin_sdk::provider_contracts::{ProviderEndpoint, ProviderRequest};
use crate::plugins::{host::PluginInfo, resource::resource_context::ResourceSnapshot};
use serde_json::Value;
use std::collections::HashMap;

pub(super) struct NativeDecorationEndpoint<'a> {
    pub id: String,
    pub directory: &'a str,
    pub entries: &'a HashMap<String, DecorationEntry>,
}

impl ProviderEndpoint for NativeDecorationEndpoint<'_> {
    fn invoke(&self, request: &ProviderRequest) -> Result<Value, String> {
        if request.method != "decorations.resolve" {
            return Err("unregistered Decoration event".into());
        }
        let decorations = super::native::resolve(&self.id, self.directory, self.entries)?;
        Ok(serde_json::json!({"decorations": decorations}))
    }
}

pub(super) struct PluginDecorationProxy<'a> {
    pub plugin: PluginInfo,
    pub directory: &'a str,
    pub resources: &'a [ResourceSnapshot],
}

impl ProviderEndpoint for PluginDecorationProxy<'_> {
    fn invoke(&self, request: &ProviderRequest) -> Result<Value, String> {
        if request.method != "decorations.resolve" {
            return Err("unregistered Decoration event".into());
        }
        crate::plugins::events::invoke_resources(
            &self.plugin,
            "decorations.resolve",
            request.params.clone(),
            self.resources,
            Some(self.directory),
        )
    }
}
