//! Core-native ContextMenuProvider implementation using the same shared
//! trait as the SDK-backed IPC proxy.
use super::builtin;
use fiex_provider_contracts::{ContextMenuProvider, ProviderError, ProviderInvocation};
use serde_json::Value;

pub(super) struct NativeMenuEndpoint<'a> {
    pub id: String,
    pub context: &'a builtin::Context,
}

impl ContextMenuProvider for NativeMenuEndpoint<'_> {
    fn resolve(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<Value>, ProviderError> {
        if ctx.method() != "contextMenu.resolve" {
            return Err(ProviderError::method_not_found(ctx.method()));
        }
        let result = builtin::resolve(&self.id, self.context).map_err(ProviderError::internal)?;
        serde_json::from_value(result["items"].clone())
            .map_err(|error| ProviderError::internal(error.to_string()))
    }

    fn execute(&self, ctx: &dyn ProviderInvocation) -> Result<Value, ProviderError> {
        if ctx.method() != "contextMenu.execute" {
            return Err(ProviderError::method_not_found(ctx.method()));
        }
        let item = ctx.params()["itemId"]
            .as_str()
            .ok_or_else(|| ProviderError::invalid_params("menu execute requires an itemId"))?;
        builtin::execute(&self.id, item, self.context).map_err(ProviderError::internal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::native_invocation::NativeProviderInvocation;
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
        let unknown = NativeProviderInvocation::new("fs.read", json!({}), &[], None);
        assert!(ContextMenuProvider::resolve(&endpoint, &unknown).is_err());
        let invalid = NativeProviderInvocation::new("contextMenu.execute", json!({}), &[], None);
        assert!(ContextMenuProvider::execute(&endpoint, &invalid).is_err());
    }
}
