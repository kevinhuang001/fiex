//! Connection lifecycle through the trusted provider registry and shared
//! ConnectionProvider trait. Credential projection, stored ownership and
//! navigation validation remain in remote/store.
use crate::plugins::{
    events::{EventRouter, Handler},
    native_invocation::NativeProviderInvocation,
};
use fiex_provider_contracts::{
    ConnectionProvider, ProviderDescriptor, ProviderError, ProviderInstances, ProviderInvocation,
    ProviderKind, ProviderMatchRule, ProviderRequest,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex, OnceLock};

struct PluginConnection {
    plugin_id: String,
}
impl PluginConnection {
    fn invoke_event(
        &self,
        ctx: &dyn ProviderInvocation,
        method: &str,
    ) -> Result<Value, ProviderError> {
        if ctx.method() != method {
            return Err(ProviderError::invalid_params(
                "ConnectionProvider method mismatch",
            ));
        }
        crate::plugins::events::invoke(&self.plugin_id, method, ctx.params().clone())
            .map_err(ProviderError::internal)
    }
}
impl ConnectionProvider for PluginConnection {
    fn validate(&self, ctx: &dyn ProviderInvocation) -> Result<Value, ProviderError> {
        self.invoke_event(ctx, "connection.validate")
    }

    fn open(&self, ctx: &dyn ProviderInvocation) -> Result<Value, ProviderError> {
        self.invoke_event(ctx, "connection.open")
    }

    fn create(&self, ctx: &dyn ProviderInvocation) -> Result<Value, ProviderError> {
        self.invoke_event(ctx, "connection.create")
    }
}

/// Persistent IPC proxies contain only plugin identity, never cached grants,
/// mount IDs or credentials. The candidate registry and Host verify all of
/// those anew before each operation. No lock survives the IPC invocation.
fn proxy_registry() -> &'static Mutex<ProviderInstances<dyn ConnectionProvider>> {
    static REGISTERED: OnceLock<Mutex<ProviderInstances<dyn ConnectionProvider>>> = OnceLock::new();
    REGISTERED.get_or_init(|| Mutex::new(ProviderInstances::new(ProviderKind::Connection)))
}

pub(crate) fn forget_plugin(plugin_id: &str) {
    if let Ok(mut providers) = proxy_registry().lock() {
        providers.unregister(&format!("plugin:{plugin_id}"));
    }
}

fn proxy_for(plugin_id: &str) -> Result<Arc<dyn ConnectionProvider>, String> {
    if plugin_id.is_empty() {
        return Err("missing plugin identity for ConnectionProvider".into());
    }
    let mut providers = proxy_registry()
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
    EventRouter::resolve_typed(
        method,
        selected,
        &request,
        |resolver, request| match &resolver.handler {
            Handler::Plugin(plugin) => {
                let invocation = NativeProviderInvocation::new(
                    request.method.clone(),
                    request.params.clone(),
                    &[],
                    None,
                );
                let provider = proxy_for(&plugin.id)?;
                match request.method.as_str() {
                    "connection.validate" => {
                        ConnectionProvider::validate(provider.as_ref(), &invocation)
                    }
                    "connection.open" => ConnectionProvider::open(provider.as_ref(), &invocation),
                    "connection.create" => {
                        ConnectionProvider::create(provider.as_ref(), &invocation)
                    }
                    _ => Err(ProviderError::method_not_found(&request.method)),
                }
                .map_err(|error| error.to_string())
            }
            Handler::Builtin(_) => Err("unknown Core connection provider".into()),
        },
        |_, value| {
            crate::plugins::events::validate_result(method, &value)?;
            Ok(value)
        },
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
        let first = proxy_for("org.example.connection-proxy").unwrap();
        let second = proxy_for("org.example.connection-proxy").unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        assert!(proxy_for("invalid/plugin/id").is_err());
        assert!(proxy_for("").is_err());
    }
}
