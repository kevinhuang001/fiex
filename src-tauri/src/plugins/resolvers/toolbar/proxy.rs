//! A Toolbar proxy persists the plugin identity, not the current selection,
//! operation scope, or installation permissions. Scoped requests are fresh.
use crate::plugins::{host::PluginInfo, resource::resource_context::ResourceSnapshot};
use fiex_provider_contracts::{
    ProviderDescriptor, ProviderEndpoint, ProviderInstances, ProviderKind, ProviderMatchRule,
    ProviderRequest,
};
use serde_json::Value;
use std::sync::{Arc, Mutex, OnceLock};

trait ScopedToolbar: Send + Sync {
    fn invoke(
        &self,
        plugin: &PluginInfo,
        resources: &[ResourceSnapshot],
        directory: &str,
        request: &ProviderRequest,
    ) -> Result<Value, String>;
}

struct IpcToolbar {
    plugin_id: String,
}

impl ScopedToolbar for IpcToolbar {
    fn invoke(
        &self,
        plugin: &PluginInfo,
        resources: &[ResourceSnapshot],
        directory: &str,
        request: &ProviderRequest,
    ) -> Result<Value, String> {
        if plugin.id != self.plugin_id || request.method != "toolbar.invoke" {
            return Err("ToolbarProvider identity or method mismatch".into());
        }
        crate::plugins::events::invoke_resources(
            plugin,
            &request.method,
            request.params.clone(),
            resources,
            Some(directory),
        )
    }
}

fn proxy(plugin_id: &str) -> Result<Arc<dyn ScopedToolbar>, String> {
    if plugin_id.is_empty() {
        return Err("missing ToolbarProvider plugin identity".into());
    }
    static REGISTERED: OnceLock<Mutex<ProviderInstances<dyn ScopedToolbar>>> = OnceLock::new();
    let registry =
        REGISTERED.get_or_init(|| Mutex::new(ProviderInstances::new(ProviderKind::Toolbar)));
    let mut providers = registry
        .lock()
        .map_err(|_| "ToolbarProvider registry unavailable".to_string())?;
    let id = format!("plugin:{plugin_id}");
    if let Some(provider) = providers.get_arc(&id) {
        return Ok(provider);
    }
    providers.register(
        ProviderDescriptor {
            id: id.clone(),
            kind: ProviderKind::Toolbar,
            priority: 50,
            r#match: ProviderMatchRule::default(),
        },
        Arc::new(IpcToolbar {
            plugin_id: plugin_id.into(),
        }),
    )?;
    providers
        .get_arc(&id)
        .ok_or_else(|| "ToolbarProvider registration failed".into())
}

struct ToolbarEndpoint<'a> {
    handler: Arc<dyn ScopedToolbar>,
    plugin: PluginInfo,
    resources: &'a [ResourceSnapshot],
    directory: &'a str,
}

impl ProviderEndpoint for ToolbarEndpoint<'_> {
    fn invoke(&self, request: &ProviderRequest) -> Result<Value, String> {
        self.handler
            .invoke(&self.plugin, self.resources, self.directory, request)
    }
}

pub(super) fn endpoint<'a>(
    plugin: &PluginInfo,
    resources: &'a [ResourceSnapshot],
    directory: &'a str,
) -> Result<Box<dyn ProviderEndpoint + 'a>, String> {
    Ok(Box::new(ToolbarEndpoint {
        handler: proxy(&plugin.id)?,
        plugin: plugin.clone(),
        resources,
        directory,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_proxy_is_persistent_without_current_operation_scope() {
        let a = proxy("org.example.toolbar").unwrap();
        let b = proxy("org.example.toolbar").unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        assert!(proxy("").is_err());
        assert!(proxy("malformed/id").is_err());
    }
}
