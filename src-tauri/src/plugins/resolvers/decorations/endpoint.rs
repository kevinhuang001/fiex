//! Unified decoration endpoint for Core-owned decorations and plugin IPC.
use super::types::DecorationEntry;
use super::proxy::ScopedDecorationProxy;
use fiex_provider_contracts::{ProviderEndpoint, ProviderRequest};
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

impl ProviderEndpoint for ScopedDecorationProxy<'_> {
    fn invoke(&self, request: &ProviderRequest) -> Result<Value, String> {
        self.resolve(request)
    }
}
