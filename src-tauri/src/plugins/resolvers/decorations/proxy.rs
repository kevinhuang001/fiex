//! Persistent Decoration IPC identity. Its operation-specific directory and
//! trusted entry snapshots live only in the ephemeral request endpoint.
use crate::plugins::{host::PluginInfo, resource::resource_context::ResourceSnapshot};
use fiex_provider_contracts::{
    ProviderDescriptor, ProviderInstances, ProviderKind, ProviderMatchRule, ProviderRequest,
};
use serde_json::Value;
use std::sync::{Arc, Mutex, OnceLock};

trait ScopedDecoration: Send + Sync {
    fn resolve(
        &self,
        plugin: &PluginInfo,
        directory: &str,
        resources: &[ResourceSnapshot],
        request: &ProviderRequest,
    ) -> Result<Value, String>;
}

struct IpcDecoration {
    plugin_id: String,
}
impl ScopedDecoration for IpcDecoration {
    fn resolve(
        &self,
        plugin: &PluginInfo,
        directory: &str,
        resources: &[ResourceSnapshot],
        request: &ProviderRequest,
    ) -> Result<Value, String> {
        if plugin.id != self.plugin_id || request.method != "decorations.resolve" {
            return Err("DecorationProvider identity or method mismatch".into());
        }
        crate::plugins::events::invoke_resources(
            plugin,
            "decorations.resolve",
            request.params.clone(),
            resources,
            Some(directory),
        )
    }
}

fn proxy(plugin_id: &str) -> Result<Arc<dyn ScopedDecoration>, String> {
    if plugin_id.is_empty() {
        return Err("missing DecorationProvider plugin identity".into());
    }
    static REGISTERED: OnceLock<Mutex<ProviderInstances<dyn ScopedDecoration>>> = OnceLock::new();
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

pub(super) struct ScopedDecorationProxy<'a> {
    handler: Arc<dyn ScopedDecoration>,
    plugin: PluginInfo,
    directory: &'a str,
    resources: &'a [ResourceSnapshot],
}
impl<'a> ScopedDecorationProxy<'a> {
    pub fn new(
        plugin: PluginInfo,
        directory: &'a str,
        resources: &'a [ResourceSnapshot],
    ) -> Result<Self, String> {
        Ok(Self {
            handler: proxy(&plugin.id)?,
            plugin,
            directory,
            resources,
        })
    }

    pub fn resolve(&self, request: &ProviderRequest) -> Result<Value, String> {
        self.handler
            .resolve(&self.plugin, self.directory, self.resources, request)
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
