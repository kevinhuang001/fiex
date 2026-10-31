use super::*;
use crate::plugin_sdk::provider_contracts::{ProviderEndpoint, ProviderRequest};

pub struct EventRouter;
impl EventRouter {
    /// Core native providers and SDK-backed IPC proxies expose
    /// the same transport-neutral endpoint, with one validation/merge policy.
    /// An endpoint factory only chooses the registered implementation; it does
    /// not select a different execution API based on where it runs.
    pub fn resolve<'a, T, R>(
        method: &str,
        providers: Vec<Resolver>,
        request: &ProviderRequest,
        mut endpoint: impl FnMut(&Resolver) -> Result<Box<dyn ProviderEndpoint + 'a>, String>,
        validate: impl FnMut(&Resolver, Value) -> Result<T, String>,
        supported: impl FnMut(&T) -> bool,
        collect: impl FnOnce(Vec<(Resolver, Result<T, String>)>) -> R,
    ) -> R {
        Self::dispatch_resolvers(
            method,
            providers,
            |provider| {
                if request.method != method {
                    return Err("provider request method does not match routed event".into());
                }
                let mut routed = request.clone();
                if let Handler::Plugin(plugin) = &provider.handler {
                    let fields = routed.params.as_object_mut()
                        .ok_or("provider parameters must be an object")?;
                    // Only the verified contribution's registered instance
                    // may select an SDK provider, never a caller-supplied ID.
                    fields.remove("providerId");
                    let prefix = format!("plugin:{}:", plugin.id);
                    if let Some(name) = provider.id.strip_prefix(&prefix) {
                        fields.insert("providerId".into(), serde_json::json!(name));
                    }
                }
                endpoint(provider)?.invoke(&routed)
            },
            validate,
            supported,
            collect,
        )
    }

    fn dispatch_resolvers<T, R>(
        method: &str,
        providers: Vec<Resolver>,
        mut invoke: impl FnMut(&Resolver) -> Result<Value, String>,
        mut validate: impl FnMut(&Resolver, Value) -> Result<T, String>,
        mut supported: impl FnMut(&T) -> bool,
        collect: impl FnOnce(Vec<(Resolver, Result<T, String>)>) -> R,
    ) -> R {
        let dispatch = event(method)
            .map(|event| event.dispatch)
            .unwrap_or(Dispatch::Merge);
        let mut outcomes = Vec::new();
        for provider in providers {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                require_event(method)?;
                let value = invoke(&provider)?;
                validate_result(method, &value)?;
                validate(&provider, value)
            }))
            .unwrap_or_else(|_| Err("provider panicked".into()));
            let accepted = result.as_ref().is_ok_and(&mut supported);
            outcomes.push((provider, result));
            if matches!(dispatch, Dispatch::Owner)
                || (matches!(dispatch, Dispatch::FirstSupported) && accepted)
            {
                break;
            }
        }
        collect(outcomes)
    }
}
