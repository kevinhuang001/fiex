//! Preview's native implementation and plugin IPC proxy implement the
//! same provider contract. Both keep the Core-owned ResourceSnapshot.
use super::{native, proxy::ScopedPreviewProxy};
use crate::plugins::resource::resource_context::ResourceSnapshot;
use fiex_provider_contracts::{ProviderEndpoint, ProviderRequest};
use serde_json::Value;
use std::sync::Arc;

pub(super) struct NativePreviewEndpoint {
    pub id: String,
    pub resource: Arc<ResourceSnapshot>,
}

impl ProviderEndpoint for NativePreviewEndpoint {
    fn invoke(&self, request: &ProviderRequest) -> Result<Value, String> {
        if request.method != "preview.open" {
            return Err("unregistered Preview event".into());
        }
        native::open(&self.id, &self.resource)
    }
}

impl ProviderEndpoint for ScopedPreviewProxy {
    fn invoke(&self, request: &ProviderRequest) -> Result<Value, String> {
        self.open(request)
    }
}
