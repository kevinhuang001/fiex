//! The Hook IPC proxy is persistent, but each invocation receives the
//! current trusted plugin record and an operation-scoped resource snapshot.
//! In particular, no mount, selection, permission grant or path is cached.
use crate::plugins::{
    host::PluginInfo, resource::resource_context::ResourceSnapshot,
};
use fiex_provider_contracts::{
    ProviderDescriptor, ProviderEndpoint, ProviderInstances, ProviderKind, ProviderMatchRule,
    ProviderRequest,
};
use serde_json::Value;
use std::sync::{Arc, Mutex, OnceLock};

trait ScopedHook: Send + Sync {
    fn invoke(
        &self,
        plugin: &PluginInfo,
        resource: &ResourceSnapshot,
        request: &ProviderRequest,
    ) -> Result<Value, String>;
}

struct IpcHook {
    plugin_id: String,
}

impl ScopedHook for IpcHook {
    fn invoke(
        &self,
        plugin: &PluginInfo,
        resource: &ResourceSnapshot,
        request: &ProviderRequest,
    ) -> Result<Value, String> {
        if plugin.id != self.plugin_id || request.method != "hooks.invoke" {
            return Err("HookProvider identity or event mismatch".into());
        }
        crate::plugins::events::invoke_resources(
            plugin,
            &request.method,
            request.params.clone(),
            std::slice::from_ref(resource),
            None,
        )
    }
}

fn proxy(plugin_id: &str) -> Result<Arc<dyn ScopedHook>, String> {
    if plugin_id.is_empty() {
        return Err("missing HookProvider plugin identity".into());
    }
    static REGISTERED: OnceLock<Mutex<ProviderInstances<dyn ScopedHook>>> = OnceLock::new();
    let registry = REGISTERED.get_or_init(|| Mutex::new(ProviderInstances::new(ProviderKind::Hook)));
    let mut providers = registry
        .lock()
        .map_err(|_| "HookProvider registry unavailable".to_string())?;
    let id = format!("plugin:{plugin_id}");
    if let Some(provider) = providers.get_arc(&id) {
        return Ok(provider);
    }
    providers.register(
        ProviderDescriptor {
            id: id.clone(),
            kind: ProviderKind::Hook,
            priority: 50,
            r#match: ProviderMatchRule::default(),
        },
        Arc::new(IpcHook {
            plugin_id: plugin_id.into(),
        }),
    )?;
    providers
        .get_arc(&id)
        .ok_or_else(|| "HookProvider registration failed".into())
}

struct HookEndpoint<'a> {
    handler: Arc<dyn ScopedHook>,
    plugin: PluginInfo,
    resource: &'a ResourceSnapshot,
}

impl ProviderEndpoint for HookEndpoint<'_> {
    fn invoke(&self, request: &ProviderRequest) -> Result<Value, String> {
        self.handler.invoke(&self.plugin, self.resource, request)
    }
}

pub(super) fn endpoint<'a>(
    plugin: &PluginInfo,
    resource: &'a ResourceSnapshot,
) -> Result<Box<dyn ProviderEndpoint + 'a>, String> {
    Ok(Box::new(HookEndpoint {
        handler: proxy(&plugin.id)?,
        plugin: plugin.clone(),
        resource,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_proxies_reuse_identity_but_reject_invalid_provider_ids() {
        let one = proxy("org.example.hook").unwrap();
        let two = proxy("org.example.hook").unwrap();
        assert!(Arc::ptr_eq(&one, &two));
        assert!(proxy("").is_err());
        assert!(proxy("untrusted/path").is_err());
    }
}
