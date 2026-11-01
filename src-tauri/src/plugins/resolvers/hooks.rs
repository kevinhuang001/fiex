//! Optional before-open hooks share the same provider registry and the
//! EventRouter's FirstSupported policy as other feature types.
use crate::plugins::{
    events::{EventRouter, ProviderCandidate},
    resource::resource_context::ResourceSnapshot,
};
use fiex_provider_contracts::{HookProvider, HookResult, ProviderError, ProviderInvocation, ProviderRequest};
use serde_json::{json, Value};
use std::sync::Arc;

mod proxy;

struct FailedHookProvider(String);
impl HookProvider for FailedHookProvider {
    fn invoke(&self, _ctx: &dyn ProviderInvocation) -> Result<HookResult, ProviderError> {
        Err(ProviderError::internal(self.0.clone()))
    }
}

pub(crate) fn forget_plugin(plugin_id: &str) {
    proxy::forget_plugin(plugin_id);
}

pub fn before_open(path: &str) -> Result<Value, String> {
    let resource = ResourceSnapshot::resolve(path)?;
    let targets = crate::plugins::events::matching::match_resources(std::slice::from_ref(&resource));
    let providers =
        crate::plugins::provider_registry::candidates("hooks.invoke", &targets, "", Vec::new())
            .into_iter()
            .filter(|provider| {
                provider.plugin.as_deref().is_some_and(|plugin|
                    plugin.extensions["hooks"]["event"] == "beforeOpen")
            });
    let providers = providers
        .map(|resolver| {
            let implementation: Arc<dyn HookProvider> = match resolver.plugin.as_deref() {
                Some(plugin) => match proxy::ScopedHookProxy::new(
                    plugin.clone(),
                    resource.clone(),
                    resolver.id.clone(),
                ) {
                    Ok(proxy) => Arc::new(proxy),
                    Err(error) => Arc::new(FailedHookProvider(error)),
                },
                None => Arc::new(FailedHookProvider(
                    "unregistered native HookProvider".into(),
                )),
            };
            ProviderCandidate {
                resolver,
                implementation,
            }
        })
        .collect();
    let call = ProviderRequest {
        method: "hooks.invoke".into(),
        params: json!({"event":"beforeOpen"}),
    };
    let invocation = crate::plugins::native_invocation::NativeProviderInvocation::new(
        call.method.clone(),
        call.params.clone(),
        std::slice::from_ref(&resource),
        None,
    );
    let result = EventRouter::resolve_shared(
        "hooks.invoke",
        providers,
        &invocation,
        |_, provider, context| provider.invoke(context).map_err(|error| error.to_string()),
        |_, result| {
            let wire = serde_json::to_value(result).map_err(|error| error.to_string())?;
            crate::plugins::events::validate_result("hooks.invoke", &wire)?;
            Ok(result)
        },
        |result| result.handled,
        |results| {
            results
                .into_iter()
                .find_map(|(_, result)| result.ok().filter(|value| value.handled))
                .unwrap_or(HookResult { handled: false })
        },
    );
    serde_json::to_value(result).map_err(|error| error.to_string())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_result_rejects_removed_preview_field() {
        assert!(serde_json::from_value::<HookResult>(json!({
            "handled": true,
            "preview": false
        }))
        .is_err());
    }
}
