//! Connection lifecycle through the trusted provider registry and shared
//! ConnectionProvider trait. Credential projection, stored ownership and
//! navigation validation remain in remote/store.
use crate::plugins::{
    events::{EventRouter, ProviderCandidate},
    native_invocation::NativeProviderInvocation,
};
use fiex_provider_contracts::{
    ConnectionOpenResult, ConnectionProvider, ConnectionValidation, MatchResource,
    ProviderDescriptor,
    ProviderError, ProviderInstances, ProviderInvocation,
    ProviderKind, ProviderMatchRule, ResourceCapabilities, ResourceLocation,
    ResourceMetadata, ResourceType,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex, OnceLock};

struct FailedConnectionProvider(String);
impl ConnectionProvider for FailedConnectionProvider {
    fn validate(&self, _ctx: &dyn ProviderInvocation) -> Result<ConnectionValidation, ProviderError> {
        Err(ProviderError::internal(self.0.clone()))
    }
    fn open(&self, _ctx: &dyn ProviderInvocation) -> Result<ConnectionOpenResult, ProviderError> {
        Err(ProviderError::internal(self.0.clone()))
    }
    fn create(&self, _ctx: &dyn ProviderInvocation) -> Result<ConnectionOpenResult, ProviderError> {
        Err(ProviderError::internal(self.0.clone()))
    }
}

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
        crate::plugins::events::invoke(&self.plugin_id, method, ctx.input()?.to_params().map_err(ProviderError::invalid_params)?)
            .map_err(ProviderError::internal)
    }
}
impl ConnectionProvider for PluginConnection {
    fn validate(&self, ctx: &dyn ProviderInvocation) -> Result<ConnectionValidation, ProviderError> {
        serde_json::from_value(self.invoke_event(ctx, "connection.validate")?)
            .map_err(|error| ProviderError::invalid_params(error.to_string()))
    }

    fn open(&self, ctx: &dyn ProviderInvocation) -> Result<ConnectionOpenResult, ProviderError> {
        serde_json::from_value(self.invoke_event(ctx, "connection.open")?)
            .map_err(|error| ProviderError::invalid_params(error.to_string()))
    }

    fn create(&self, ctx: &dyn ProviderInvocation) -> Result<ConnectionOpenResult, ProviderError> {
        serde_json::from_value(self.invoke_event(ctx, "connection.create")?)
            .map_err(|error| ProviderError::invalid_params(error.to_string()))
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

struct ScopedConnectionProxy {
    handler: Arc<dyn ConnectionProvider>,
    plugin_id: String,
    provider_id: String,
}

impl ScopedConnectionProxy {
    fn new(plugin_id: &str, provider_id: &str) -> Result<Self, String> {
        Ok(Self {
            handler: proxy_for(plugin_id)?,
            plugin_id: plugin_id.into(),
            provider_id: provider_id.into(),
        })
    }

    fn invocation(&self, ctx: &dyn ProviderInvocation) -> Result<NativeProviderInvocation, ProviderError> {
        let request = crate::plugins::events::bind_provider_context(
            &self.plugin_id,
            &self.provider_id,
            ctx.request(),
        )
        .map_err(ProviderError::invalid_params)?;
        Ok(NativeProviderInvocation::from_request_context(request, &[], None))
    }
}

impl ConnectionProvider for ScopedConnectionProxy {
    fn validate(&self, ctx: &dyn ProviderInvocation) -> Result<ConnectionValidation, ProviderError> {
        self.handler.validate(&self.invocation(ctx)?)
    }

    fn open(&self, ctx: &dyn ProviderInvocation) -> Result<ConnectionOpenResult, ProviderError> {
        self.handler.open(&self.invocation(ctx)?)
    }

    fn create(&self, ctx: &dyn ProviderInvocation) -> Result<ConnectionOpenResult, ProviderError> {
        self.handler.create(&self.invocation(ctx)?)
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
    if !matches!(
        method,
        "connection.validate" | "connection.open" | "connection.create"
    ) {
        return Err("unregistered connection event".into());
    }
    let owner = ResourceLocation {
        resource_type: ResourceType::Vfs,
        provider_id: format!("plugin:{plugin_id}:{kind}"),
        mount_id: Some(mount_id.into()),
    };
    let targets = [MatchResource {
        metadata: Some(ResourceMetadata {
            kind: "connection".into(),
            ..Default::default()
        }),
        view: Some(owner.clone()),
        storage: Some(owner),
        capabilities: ResourceCapabilities::default(),
    }];
    let selected = crate::plugins::provider_registry::candidates(
        method, &targets, "", Vec::new()
    ).into_iter().filter(|resolver|
        resolver.plugin.as_deref().is_some_and(|plugin| plugin.id == plugin_id)
    ).map(|resolver| {
        let implementation: Arc<dyn ConnectionProvider> = match resolver.plugin.as_deref() {
            Some(plugin) => ScopedConnectionProxy::new(&plugin.id, &resolver.id)
                .map(|proxy| Arc::new(proxy) as Arc<dyn ConnectionProvider>)
                .unwrap_or_else(|error| Arc::new(FailedConnectionProvider(error))),
            None => Arc::new(FailedConnectionProvider(
                "unregistered native ConnectionProvider".into(),
            )),
        };
        ProviderCandidate { resolver, implementation }
    }).collect::<Vec<_>>();
    let request = ProviderRequest {
        method: method.into(),
        params,
    };
    let invocation = NativeProviderInvocation::new(
        request.method.clone(),
        request.params.clone(),
        &[],
        None,
    );
    EventRouter::resolve_shared(
        method,
        selected,
        &invocation,
        |_, provider, invocation| match invocation.method() {
            "connection.validate" => provider.validate(invocation).and_then(|result| {
                serde_json::to_value(result)
                    .map_err(|error| ProviderError::internal(error.to_string()))
            }),
            "connection.open" => provider.open(invocation).and_then(|result| {
                serde_json::to_value(result)
                    .map_err(|error| ProviderError::internal(error.to_string()))
            }),
            "connection.create" => provider.create(invocation).and_then(|result| {
                serde_json::to_value(result)
                    .map_err(|error| ProviderError::internal(error.to_string()))
            }),
            method => Err(ProviderError::method_not_found(method)),
        }
        .map_err(|error| error.to_string()),
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
