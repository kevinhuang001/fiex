//! One ContextMenu execution contract for Core-native and IPC providers.
//!
//! Both transports implement the SDK's ProviderEndpoint. Core owns the menu
//! session, resource scope and output validation independently of transport.
use super::{builtin, proxy::ScopedMenuProxy};
use fiex_provider_contracts::{ProviderEndpoint, ProviderRequest};
use serde_json::Value;

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

impl ProviderEndpoint for ScopedMenuProxy {
    fn invoke(&self, request: &ProviderRequest) -> Result<Value, String> {
        self.call(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn native_endpoint_rejects_unknown_methods_and_missing_item() {
        let ctx = builtin::Context::for_test_caps(
            "/tmp",
            Vec::new(),
            Vec::new(),
            true,
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
