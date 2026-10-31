//! One ContextMenu execution contract for Core-native and IPC providers.
//!
//! Both transports implement the SDK's ProviderEndpoint. Core owns the menu
//! session, resource scope and output validation independently of transport.
use super::builtin;
use crate::plugins::{host::PluginInfo, resource::resource_context::ResourceSnapshot};
use fiex_provider_contracts::{ProviderEndpoint, ProviderRequest};
use serde_json::{json, Value};
use std::sync::Arc;

pub(super) struct NativeMenuEndpoint<'a> {
    pub id: String,
    pub context: &'a builtin::Context,
}

impl ProviderEndpoint for NativeMenuEndpoint<'_> {
    fn invoke(&self, request: &ProviderRequest) -> Result<Value, String> {
        match request.method.as_str() {
            "contextMenu.resolve" => builtin::resolve(&self.id, self.context),
            "contextMenu.execute" => {
                let item = request.params["itemId"]
                    .as_str()
                    .ok_or("menu execute requires an itemId")?;
                builtin::execute(&self.id, item, self.context)
            }
            _ => Err("unsupported ContextMenu event".into()),
        }
    }
}

pub(super) struct PluginMenuProxy {
    pub plugin: PluginInfo,
    pub directory: String,
    pub resources: Arc<Vec<ResourceSnapshot>>,
}

impl ProviderEndpoint for PluginMenuProxy {
    fn invoke(&self, request: &ProviderRequest) -> Result<Value, String> {
        match request.method.as_str() {
            "contextMenu.resolve" => {
                let session_id = crate::plugins::mounts::new_id()?;
                let mut params = request.params.clone();
                let object = params.as_object_mut().ok_or("invalid menu parameters")?;
                object.insert("menuSessionId".into(), json!(session_id));
                object.insert(
                    "interactionId".into(),
                    json!(crate::plugins::mounts::new_id()?),
                );
                let mut result = crate::plugins::events::invoke_resources(
                    &self.plugin,
                    "contextMenu.resolve",
                    params,
                    &self.resources,
                    Some(&self.directory),
                )?;
                result["menuSessionId"] = json!(session_id);
                Ok(result)
            }
            "contextMenu.execute" => {
                let params = request.params.clone();
                let mut result = crate::plugins::events::call_owner(
                    "contextMenu.execute",
                    &self.plugin.id,
                    params,
                    &self.resources,
                    Some(&self.directory),
                )?;
                // Only Core handlers can issue filesystem actions directly.
                if matches!(result["kind"].as_str(), Some("coreAction" | "remoteAction")) {
                    return Err("plugin returned a reserved action kind".into());
                }
                if result["kind"] == "panel" {
                    let entry = result["entry"]
                        .as_str()
                        .ok_or("panel result missing entry")?;
                    let asset = crate::plugins::host::plugin_view_asset(&self.plugin.id, entry)?;
                    let object = result.as_object_mut().ok_or("invalid panel result")?;
                    object.remove("entry");
                    object.insert("assetPath".into(), json!(asset));
                }
                Ok(result)
            }
            _ => Err("unsupported ContextMenu event".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_endpoint_rejects_unknown_methods_and_missing_item() {
        let ctx = builtin::Context::for_test_caps(
            "/tmp",
            Vec::new(),
            Vec::new(),
            true,
            false,
            Default::default(),
        );
        let endpoint = NativeMenuEndpoint {
            id: "builtin:opening".into(),
            context: &ctx,
        };
        let unknown = ProviderRequest {
            method: "fs.read".into(),
            params: json!({}),
        };
        assert!(endpoint.invoke(&unknown).is_err());
        let invalid = ProviderRequest {
            method: "contextMenu.execute".into(),
            params: json!({}),
        };
        assert!(endpoint.invoke(&invalid).is_err());
    }
}
