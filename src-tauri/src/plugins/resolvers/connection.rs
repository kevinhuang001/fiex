//! Connection lifecycle through the same trusted provider candidate registry
//! and SDK endpoint used by every other Core feature. Credential projection,
//! stored ownership and navigation validation remain in remote/store.
use crate::plugin_sdk::provider_contracts::{ProviderEndpoint, ProviderRequest};
use crate::plugins::events::{EventRouter, Handler};
use serde_json::{json, Value};

struct PluginConnection {
    plugin_id: String,
}
impl ProviderEndpoint for PluginConnection {
    fn invoke(&self, request: &ProviderRequest) -> Result<Value, String> {
        if !matches!(
            request.method.as_str(),
            "connection.validate" | "connection.open" | "connection.create"
        ) {
            return Err("unsupported connection event".into());
        }
        crate::plugins::events::invoke(
            &self.plugin_id,
            &request.method,
            request.params.clone(),
        )
    }
}

/// The record and mount were validated and allocated by Core; its identity
/// and provider kind never come from a plugin response or opaque UI metadata.
pub fn invoke(
    plugin_id: &str,
    kind: &str,
    mount_id: &str,
    method: &str,
    params: Value,
) -> Result<Value, String> {
    if kind.is_empty() || mount_id.is_empty() || plugin_id.is_empty() {
        return Err("invalid connection identity".into());
    }
    if !matches!(method, "connection.validate" | "connection.open" | "connection.create") {
        return Err("unregistered connection event".into());
    }
    let metadata = [json!({
        "kind":"connection",
        "view":{
            "type":"vfs",
            "providerId":format!("plugin:{plugin_id}:{kind}"),
            "mountId":mount_id,
        },
        "operations":[]
    })];
    let selected = crate::plugins::provider_registry::candidates(
        method, &metadata, "", Vec::new()
    ).into_iter().filter(|resolver|
        matches!(&resolver.handler, Handler::Plugin(plugin) if plugin.id == plugin_id)
    ).collect();
    let request = ProviderRequest { method:method.into(), params };
    EventRouter::resolve(
        method,
        selected,
        &request,
        |resolver| match &resolver.handler {
            Handler::Plugin(plugin) => Ok(Box::new(PluginConnection {
                plugin_id: plugin.id.clone(),
            }) as Box<dyn ProviderEndpoint>),
            Handler::Builtin(_) => Err("unknown Core connection provider".into()),
        },
        |_, value| Ok(value),
        |_| true,
        |results| results.into_iter().next()
            .ok_or_else(|| "connection provider unavailable or not matched".to_string())?.1,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lifecycle_requires_registered_method_and_connection_identity() {
        assert!(invoke("", "drive", "mount", "connection.open", json!({})).is_err());
        assert!(invoke("org.example", "drive", "mount", "vfs.list", json!({})).is_err());
    }
}
