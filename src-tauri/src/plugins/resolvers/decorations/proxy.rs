//! Persistent DecorationProvider IPC identity. Its operation-specific directory
//! and trusted entry snapshots live only in the invocation context.
use super::invocation::{self, CoreDecorationInvocation};
use super::types::DecorationEntry;
use crate::plugins::{host::PluginInfo, resource::resource_context::ResourceSnapshot};
use fiex_provider_contracts::{
    DecorationProvider, ProviderDescriptor, ProviderError, ProviderInstances, ProviderInvocation,
    ProviderKind, ProviderMatchRule, ProviderRequest,
};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};

struct IpcDecoration {
    plugin_id: String,
}
impl DecorationProvider for IpcDecoration {
    fn resolve(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<Value>, ProviderError> {
        let services = invocation::services(ctx).map_err(ProviderError::internal)?;
        let plugin = services
            .plugin
            .as_ref()
            .ok_or_else(|| ProviderError::internal("missing DecorationProvider plugin"))?;
        if plugin.id != self.plugin_id || ctx.method() != "decorations.resolve" {
            return Err(ProviderError::invalid_params(
                "DecorationProvider identity or method mismatch",
            ));
        }
        let result = crate::plugins::events::invoke_resources(
            plugin,
            ctx.method(),
            ctx.params().clone(),
            services.resources.as_slice(),
            Some(services.directory.as_str()),
        )
        .map_err(ProviderError::internal)?;
        serde_json::from_value(result["decorations"].clone())
            .map_err(|error| ProviderError::invalid_params(error.to_string()))
    }
}

fn proxy(plugin_id: &str) -> Result<Arc<dyn DecorationProvider>, String> {
    if plugin_id.is_empty() {
        return Err("missing DecorationProvider plugin identity".into());
    }
    static REGISTERED: OnceLock<Mutex<ProviderInstances<dyn DecorationProvider>>> =
        OnceLock::new();
    let registry =
        REGISTERED.get_or_init(|| Mutex::new(ProviderInstances::new(ProviderKind::Decoration)));
    let mut registry = registry
        .lock()
        .map_err(|_| "DecorationProvider registry unavailable".to_string())?;
    let id = format!("plugin:{plugin_id}");
    if let Some(instance) = registry.get_arc(&id) {
        return Ok(instance);
    }
    registry.register(
        ProviderDescriptor {
            id: id.clone(),
            kind: ProviderKind::Decoration,
            priority: 50,
            r#match: ProviderMatchRule::default(),
        },
        Arc::new(IpcDecoration {
            plugin_id: plugin_id.into(),
        }),
    )?;
    registry
        .get_arc(&id)
        .ok_or_else(|| "DecorationProvider registration failed".into())
}

pub(super) struct ScopedDecorationProxy {
    handler: Arc<dyn DecorationProvider>,
    plugin: PluginInfo,
    directory: String,
    entries: Arc<HashMap<String, DecorationEntry>>,
    resources: Arc<Vec<ResourceSnapshot>>,
}
impl ScopedDecorationProxy {
    pub fn new(
        plugin: PluginInfo,
        directory: &str,
        entries: Arc<HashMap<String, DecorationEntry>>,
        resources: Arc<Vec<ResourceSnapshot>>,
    ) -> Result<Self, String> {
        Ok(Self {
            handler: proxy(&plugin.id)?,
            plugin,
            directory: directory.into(),
            entries,
            resources,
        })
    }

    pub fn resolve(&self, request: &ProviderRequest) -> Result<Value, String> {
        let invocation = CoreDecorationInvocation::plugin(
            self.directory.clone(),
            self.entries.clone(),
            self.resources.clone(),
            self.plugin.clone(),
            request,
        );
        self.handler
            .resolve(&invocation)
            .map_err(|error| error.to_string())
            .and_then(|decorations| {
                serde_json::to_value(decorations).map_err(|error| error.to_string())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoped_decorations_reuse_identity_not_current_selection() {
        let a = proxy("org.example.decoration").unwrap();
        let b = proxy("org.example.decoration").unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        assert!(proxy("").is_err());
        assert!(proxy("invalid/provider").is_err());
    }
}
