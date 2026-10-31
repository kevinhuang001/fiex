//! Optional before-open hooks share the same provider registry and the
//! EventRouter's FirstSupported policy as other feature types.
use crate::plugins::{
    events::{EventRouter, Handler},
    resource::resource_context::ResourceSnapshot,
};
use fiex_provider_contracts::ProviderRequest;
use serde_json::{json, Value};

mod proxy;

pub fn before_open(path: &str) -> Result<Value, String> {
    let resource = ResourceSnapshot::resolve(path)?;
    let metadata = crate::plugins::events::resource_metadata(std::slice::from_ref(&resource));
    let providers =
        crate::plugins::provider_registry::candidates("hooks.invoke", &metadata, "", Vec::new())
            .into_iter()
            .filter(|provider| {
                matches!(&provider.handler, Handler::Plugin(plugin)
            if plugin.extensions["hooks"]["event"] == "beforeOpen")
            })
            .collect();
    let call = ProviderRequest {
        method: "hooks.invoke".into(),
        params: json!({"event":"beforeOpen"}),
    };
    Ok(EventRouter::resolve(
        "hooks.invoke",
        providers,
        &call,
        |resolver| match &resolver.handler {
            Handler::Plugin(plugin) => proxy::endpoint(plugin, &resource),
            Handler::Builtin(_) => Err("unregistered native HookProvider".into()),
        },
        |_, value| validate_hook_reply(value),
        |value| value["handled"] == true,
        |results| {
            results
                .into_iter()
                .find_map(|(_, result)| result.ok().filter(|value| value["handled"] == true))
                .unwrap_or_else(|| json!({"handled":false}))
        },
    ))
}

/// A hook can stop the FirstSupported chain only with a well-formed reply.
/// Invalid plugin replies must not silently masquerade as "not handled".
fn validate_hook_reply(value: Value) -> Result<Value, String> {
    if value.get("handled").and_then(Value::as_bool).is_none() {
        return Err("invalid hook reply: handled must be a boolean".into());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_untyped_hook_results() {
        assert!(validate_hook_reply(json!({})).is_err());
        assert!(validate_hook_reply(json!({"handled":"true"})).is_err());
        assert!(validate_hook_reply(json!(null)).is_err());
    }

    #[test]
    fn accepts_explicit_handled_states() {
        assert_eq!(
            validate_hook_reply(json!({"handled":true})).unwrap()["handled"],
            true
        );
        assert_eq!(
            validate_hook_reply(json!({"handled":false})).unwrap()["handled"],
            false
        );
    }
}
