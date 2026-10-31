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
                let routed = match &provider.handler {
                    Handler::Builtin(_) => request.clone(),
                    Handler::Plugin(plugin) => {
                        bound_plugin_request(request, &plugin.id, &provider.id)?
                    }
                };
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

fn bound_plugin_request(
    request: &ProviderRequest,
    plugin_id: &str,
    registered_id: &str,
) -> Result<ProviderRequest, String> {
    let mut routed = request.clone();
    let fields = routed
        .params
        .as_object_mut()
        .ok_or("provider parameters must be an object")?;
    fields.remove("providerId");
    let prefix = format!("plugin:{plugin_id}:");
    if let Some(id) = registered_id.strip_prefix(&prefix) {
        if id.is_empty() {
            return Err("empty registered provider instance ID".into());
        }
        fields.insert("providerId".into(), serde_json::json!(id));
    } else if registered_id != format!("plugin:{plugin_id}") {
        // Never execute an endpoint with an identity from a different plugin.
        return Err("provider instance does not belong to selected plugin".into());
    }
    Ok(routed)
}

#[cfg(test)]
mod instance_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn core_binds_selected_instance_and_erases_forged_provider_ids() {
        let forged = ProviderRequest {
            method: "info.resolve".into(),
            params: json!({"providerId":"forged","other":"ok"}),
        };
        let named = bound_plugin_request(
            &forged,
            "org.example.viewer",
            "plugin:org.example.viewer:media",
        )
        .unwrap();
        assert_eq!(named.params["providerId"], "media");
        assert_eq!(named.params["other"], "ok");
        let single =
            bound_plugin_request(&forged, "org.example.viewer", "plugin:org.example.viewer")
                .unwrap();
        assert!(single.params.get("providerId").is_none());
        assert!(bound_plugin_request(&forged, "org.example.viewer", "plugin:another.plugin:media").is_err());
        assert!(bound_plugin_request(&forged, "org.example.viewer", "plugin:org.example.viewer:").is_err());
        assert!(bound_plugin_request(
            &ProviderRequest {
                method: "info.resolve".into(),
                params: json!(null)
            },
            "org.example.viewer",
            "plugin:org.example.viewer"
        )
        .is_err());
    }
}
