//! Before-open hooks use first-supported dispatch; after-open hooks broadcast
//! only after Core confirms an open was accepted.
use crate::plugins::{
    events::{EventRouter, ProviderCandidate},
    resource::resource_context::ResourceSnapshot,
};
use fiex_provider_contracts::{
    HookProvider, HookResult, ProviderError, ProviderInvocation, ProviderRequest,
};
use serde_json::{json, Value};
use std::sync::Arc;

mod native;
mod proxy;

pub(crate) fn register_native(
    registry: &mut crate::plugins::provider_registry::native::NativeProviderRegistry<
        dyn HookProvider,
    >,
) {
    native::register(registry);
}

struct FailedHookProvider(String);
impl HookProvider for FailedHookProvider {
    fn invoke(&self, _ctx: &dyn ProviderInvocation) -> Result<HookResult, ProviderError> {
        Err(ProviderError::internal(self.0.clone()))
    }
}

fn candidates(
    resource: &ResourceSnapshot,
    event: &str,
) -> Vec<ProviderCandidate<dyn HookProvider>> {
    let targets = crate::plugins::events::matching::match_resources(std::slice::from_ref(resource));
    let native = native::candidates(resource, event);
    crate::plugins::provider_registry::candidates("hooks.invoke", &targets, "", native)
        .into_iter()
        .filter(|provider| {
            if let Some(plugin) = provider.plugin.as_deref() {
                plugin.extensions["hooks"]["event"] == event
            } else {
                native::provider(&provider.id).is_some()
            }
        })
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
                None => native::provider(&resolver.id).unwrap_or_else(|| {
                    Arc::new(FailedHookProvider(
                        "unregistered native HookProvider".into(),
                    ))
                }),
            };
            ProviderCandidate {
                resolver,
                implementation,
            }
        })
        .collect()
}

pub(crate) fn forget_plugin(plugin_id: &str) {
    proxy::forget_plugin(plugin_id);
}

pub fn before_open(path: &str) -> Result<Value, String> {
    let resource = crate::plugins::resource::resource_context::resolve(path)?;
    let providers = candidates(&resource, "beforeOpen");
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
        |resolver, provider, context| {
            if resolver.plugin.is_none() {
                let native = native::invocation(
                    &resolver.id,
                    call.method.clone(),
                    call.params.clone(),
                    std::slice::from_ref(&resource),
                )?;
                provider.invoke(&native).map_err(|error| error.to_string())
            } else {
                provider.invoke(context).map_err(|error| error.to_string())
            }
        },
        |_, result| {
            let wire = serde_json::to_value(result).map_err(|error| error.to_string())?;
            crate::plugins::events::validate_result("hooks.invoke", &wire)?;
            if result.handled {
                complete_open(&resource, true);
            }
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

pub fn after_open(resource: &ResourceSnapshot) {
    let providers = candidates(resource, "afterOpen");
    if providers.is_empty() {
        return;
    }
    let call = ProviderRequest {
        method: "hooks.invoke".into(),
        params: json!({"event":"afterOpen"}),
    };
    let invocation = crate::plugins::native_invocation::NativeProviderInvocation::new(
        call.method.clone(),
        call.params.clone(),
        std::slice::from_ref(resource),
        None,
    );
    EventRouter::resolve_merge_parallel(
        "hooks.invoke",
        providers,
        &invocation,
        |resolver, provider, context| {
            if resolver.plugin.is_none() {
                let native = native::invocation(
                    &resolver.id,
                    call.method.clone(),
                    call.params.clone(),
                    std::slice::from_ref(resource),
                )?;
                provider.invoke(&native).map_err(|error| error.to_string())
            } else {
                provider.invoke(context).map_err(|error| error.to_string())
            }
        },
        |_, result| {
            let wire = serde_json::to_value(result).map_err(|error| error.to_string())?;
            crate::plugins::events::validate_result("hooks.invoke", &wire)?;
            Ok(result)
        },
        |outcomes| {
            for (provider, result) in outcomes {
                let Err(error) = result else { continue };
                if let Some(plugin) = provider.plugin.as_deref() {
                    let _ = crate::plugins::errors::report(
                        &plugin.id,
                        "host",
                        &format!("afterOpen Hook failed: {error}"),
                        Some(&error),
                    );
                } else {
                    crate::infrastructure::logging::record_core(
                        "error",
                        "provider.hook",
                        &format!("afterOpen provider={} failed: {error}", provider.id),
                    );
                }
            }
        },
    );
}

/// The single completion boundary for every successful open path. Failed and
/// cancelled opens never publish an afterOpen event.
pub(crate) fn complete_open(resource: &ResourceSnapshot, accepted: bool) {
    if accepted {
        after_open(resource);
    }
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
