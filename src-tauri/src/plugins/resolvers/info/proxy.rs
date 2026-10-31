//! Shared InfoProvider IPC instance registry. A proxy owns only the plugin
//! identifier: live plugin state, scoped resources and operation grants are
//! supplied anew on each request and never stored in the registry.
use crate::plugins::{host::PluginInfo, resource::resource_context::ResourceSnapshot};
use fiex_provider_contracts::{
    ProviderDescriptor, ProviderInstances, ProviderKind, ProviderMatchRule, ProviderRequest,
};
use serde_json::Value;
use std::sync::{Arc, Mutex, OnceLock};

trait ScopedInfo: Send + Sync {
    fn resolve(
        &self,
        plugin: &PluginInfo,
        resource: &ResourceSnapshot,
        request: &ProviderRequest,
    ) -> Result<Value, String>;
}

struct IpcInfo {
    plugin_id: String,
}

impl ScopedInfo for IpcInfo {
    fn resolve(
        &self,
        plugin: &PluginInfo,
        resource: &ResourceSnapshot,
        request: &ProviderRequest,
    ) -> Result<Value, String> {
        if self.plugin_id != plugin.id || request.method != "info.resolve" {
            return Err("InfoProvider identity or event mismatch".into());
        }
        crate::plugins::events::invoke_resources(
            plugin,
            "info.resolve",
            request.params.clone(),
            std::slice::from_ref(resource),
            None,
        )
    }
}

fn proxy(plugin_id: &str) -> Result<Arc<dyn ScopedInfo>, String> {
    if plugin_id.is_empty() {
        return Err("missing InfoProvider plugin identity".into());
    }
    static REGISTERED: OnceLock<Mutex<ProviderInstances<dyn ScopedInfo>>> = OnceLock::new();
    let registry = REGISTERED.get_or_init(|| Mutex::new(ProviderInstances::new(ProviderKind::Info)));
    let mut registered = registry
        .lock()
        .map_err(|_| "InfoProvider registry unavailable".to_string())?;
    let id = format!("plugin:{plugin_id}");
    if let Some(instance) = registered.get_arc(&id) {
        return Ok(instance);
    }
    registered.register(
        ProviderDescriptor {
            id: id.clone(),
            kind: ProviderKind::Info,
            priority: 50,
            r#match: ProviderMatchRule::default(),
        },
        Arc::new(IpcInfo {
            plugin_id: plugin_id.into(),
        }),
    )?;
    registered
        .get_arc(&id)
        .ok_or_else(|| "InfoProvider registration failed".into())
}

pub(super) struct ScopedInfoProxy {
    handler: Arc<dyn ScopedInfo>,
    plugin: PluginInfo,
    resource: Arc<ResourceSnapshot>,
}

impl ScopedInfoProxy {
    pub fn new(plugin: PluginInfo, resource: Arc<ResourceSnapshot>) -> Result<Self, String> {
        let handler = proxy(&plugin.id)?;
        Ok(Self {
            handler,
            plugin,
            resource,
        })
    }

    pub fn resolve(&self, request: &ProviderRequest) -> Result<Value, String> {
        self.handler.resolve(&self.plugin, &self.resource, request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_proxies_reuse_identities_without_retaining_resources_or_grants() {
        let a = proxy("org.example.info").unwrap();
        let b = proxy("org.example.info").unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        assert!(proxy("").is_err());
        assert!(proxy("bad/id").is_err());
    }
}
