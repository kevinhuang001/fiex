//! Optional before-open hooks share the same provider registry and the
//! EventRouter's FirstSupported policy as other feature types.
use crate::plugins::{
    events::{EventRouter, Handler},
    resource::resource_context::ResourceSnapshot,
};
use fiex_provider_contracts::{HookResult, ProviderRequest};
use serde_json::{json, Value};

mod proxy;

pub(crate) fn forget_plugin(plugin_id: &str) {
    proxy::forget_plugin(plugin_id);
}

pub fn before_open(path: &str) -> Result<Value, String> {
    let resource = ResourceSnapshot::resolve(path)?;
    let match_targets =
        crate::plugins::events::matching::match_resources(std::slice::from_ref(&resource));
    let providers = crate::plugins::provider_registry::candidates(
        "hooks.invoke",
        &match_targets,
        "",
        Vec::new(),
    )
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
    let result = EventRouter::resolve_typed(
        "hooks.invoke",
        providers,
        &call,
        |resolver, request| match &resolver.handler {
            Handler::Plugin(plugin) => proxy::invoke(plugin, &resource, request),
            Handler::Builtin(_) => Err("unregistered native HookProvider".into()),
        },
        |_, result| {
            let value = serde_json::to_value(result).map_err(|error| error.to_string())?;
            crate::plugins::events::validate_result("hooks.invoke", &value)?;
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
    fn rejects_legacy_hook_reply_fields() {
        assert!(
            serde_json::from_value::<HookResult>(json!({"handled":true,"preview":false})).is_err()
        );
    }
}
