//! Preview's native implementation and plugin IPC proxy implement the
//! same provider contract. Both keep the Core-owned ResourceSnapshot.
use super::{
    builtin::{builtin_view, Builtin},
    session::open_plugin_resource,
};
use crate::plugin_sdk::provider_contracts::{ProviderEndpoint, ProviderRequest};
use crate::plugins::{host::PluginInfo, resource::resource_context::ResourceSnapshot};
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
        let kind = [
            Builtin::Text,
            Builtin::Image,
            Builtin::Media,
            Builtin::Pdf,
            Builtin::Fallback,
        ]
        .into_iter()
        .find(|builtin| builtin.id() == self.id)
        .ok_or("unknown native Preview provider")?;
        let path = if kind == Builtin::Fallback {
            self.resource.local.clone()
        } else {
            self.resource.export_path()?
        };
        builtin_view(kind, &path, &self.resource.display)
    }
}

pub(super) struct PluginPreviewProxy {
    pub plugin: PluginInfo,
    pub resource: Arc<ResourceSnapshot>,
}

impl ProviderEndpoint for PluginPreviewProxy {
    fn invoke(&self, request: &ProviderRequest) -> Result<Value, String> {
        if request.method != "preview.open" {
            return Err("unregistered Preview event".into());
        }
        // Keep preview session creation, operation scope, returned HTML
        // package validation and owner binding in the existing secure service.
        let response = open_plugin_resource(
            &self.resource,
            &self.plugin,
            request.params["providerId"].as_str(),
        )?;
        let mut view = response["view"].clone();
        if let Some(object) = view.as_object_mut() {
            for key in [
                "sessionId",
                "context",
                "pluginId",
                "contributionId",
                "timeoutMs",
            ] {
                object.insert(key.into(), response[key].clone());
            }
        }
        Ok(view)
    }
}
