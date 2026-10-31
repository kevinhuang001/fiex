//! Connection lifecycle through the same trusted provider candidate registry
//! and SDK endpoint used by every other Core feature. Credential projection,
//! stored ownership and navigation validation remain in remote/store.
use crate::plugins::events::{EventRouter, Handler};
use fiex_provider_contracts::{
    ProviderDescriptor, ProviderEndpoint, ProviderInstances, ProviderKind, ProviderMatchRule,
    ProviderRequest,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex, OnceLock};

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
        crate::plugins::events::invoke(&self.plugin_id, &request.method, request.params.clone())
    }
}

/// Persistent IPC proxies contain only plugin identity, never cached grants,
/// mount IDs or credentials. The candidate registry and Host verify all of
/// those anew before each operation. No lock survives the IPC invocation.
fn endpoint_for(plugin_id: &str) -> Result<Arc<dyn ProviderEndpoint>, String> {
    if plugin_id.is_empty() {
        return Err("missing plugin identity for ConnectionProvider".into());
    }
    static REGISTERED: OnceLock<Mutex<ProviderInstances<dyn ProviderEndpoint>>> = OnceLock::new();
    let registry =
        REGISTERED.get_or_init(|| Mutex::new(ProviderInstances::new(ProviderKind::Connection)));
    let mut providers = registry
        .lock()
        .map_err(|_| "connection provider registry unavailable".to_string())?;
    let id = format!("plugin:{plugin_id}");
    if let Some(existing) = providers.get_arc(&id) {
        return Ok(existing);
    }
    providers.register(
        ProviderDescriptor {
            id: id.clone(),
            kind: ProviderKind::Connection,
            priority: 50,
            r#match: ProviderMatchRule::default(),
        },
        Arc::new(PluginConnection {
            plugin_id: plugin_id.into(),
        }),
    )?;
    providers
        .get_arc(&id)
        .ok_or_else(|| "connection proxy registration failed".into())
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
    if !matches!(
        method,
        "connection.validate" | "connection.open" | "connection.create"
    ) {
        return Err("unregistered connection event".into());
    }
    let owner = json!({
        "type":"vfs",
        "providerId":format!("plugin:{plugin_id}:{kind}"),
        "mountId":mount_id,
    });
    let metadata = [json!({
        "kind":"connection",
        "view": owner,
        "storage": owner,
        "operations":[]
    })];
    let selected = crate::plugins::provider_registry::candidates(
        method, &metadata, "", Vec::new()
    ).into_iter().filter(|resolver|
        matches!(&resolver.handler, Handler::Plugin(plugin) if plugin.id == plugin_id)
    ).collect();
    let request = ProviderRequest {
        method: method.into(),
        params,
    };
    EventRouter::resolve(
        method,
        selected,
        &request,
        |resolver| match &resolver.handler {
            Handler::Plugin(plugin) => {
                Ok(Box::new(endpoint_for(&plugin.id)?) as Box<dyn ProviderEndpoint>)
            }
            Handler::Builtin(_) => Err("unknown Core connection provider".into()),
        },
        |_, value| Ok(value),
        |_| true,
        |results| {
            results
                .into_iter()
                .next()
                .ok_or_else(|| "connection provider unavailable or not matched".to_string())?
                .1
        },
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

    #[test]
    fn connection_ipc_proxy_is_registered_once_without_retaining_mount_data() {
        let first = endpoint_for("org.example.connection-proxy").unwrap();
        let second = endpoint_for("org.example.connection-proxy").unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        assert!(endpoint_for("invalid/plugin/id").is_err());
        assert!(endpoint_for("").is_err());
    }
}
