//! Persistent Preview IPC identities with ephemeral trusted resources.
//! Session creation, HTML validation and capability checks remain exclusively
//! in Core's established preview session service.
use super::session::open_plugin_resource;
use crate::plugins::{host::PluginInfo, resource::resource_context::ResourceSnapshot};
use fiex_provider_contracts::{
    ProviderDescriptor, ProviderInstances, ProviderKind, ProviderMatchRule, ProviderRequest,
};
use serde_json::Value;
use std::sync::{Arc, Mutex, OnceLock};

trait ScopedPreview: Send + Sync {
    fn open(
        &self,
        plugin: &PluginInfo,
        resource: &ResourceSnapshot,
        request: &ProviderRequest,
    ) -> Result<Value, String>;
}

struct IpcPreview {
    plugin_id: String,
}
impl ScopedPreview for IpcPreview {
    fn open(
        &self,
        plugin: &PluginInfo,
        resource: &ResourceSnapshot,
        request: &ProviderRequest,
    ) -> Result<Value, String> {
        if self.plugin_id != plugin.id || request.method != "preview.open" {
            return Err("PreviewProvider identity or event mismatch".into());
        }
        let response = open_plugin_resource(
            resource,
            plugin,
            request.params["providerId"].as_str(),
        )?;
        let mut view = response["view"].clone();
        if let Some(object) = view.as_object_mut() {
            for key in [
                "sessionId",
                "context",
                "pluginId",
                "contributionId",
                "timeoutMs",
            ] {
                object.insert(key.into(), response[key].clone());
            }
        }
        Ok(view)
    }
}

fn proxy(plugin_id: &str) -> Result<Arc<dyn ScopedPreview>, String> {
    if plugin_id.is_empty() {
        return Err("missing PreviewProvider plugin identity".into());
    }
    static REGISTERED: OnceLock<Mutex<ProviderInstances<dyn ScopedPreview>>> = OnceLock::new();
    let registry =
        REGISTERED.get_or_init(|| Mutex::new(ProviderInstances::new(ProviderKind::Preview)));
    let mut providers = registry
        .lock()
        .map_err(|_| "PreviewProvider registry unavailable".to_string())?;
    let id = format!("plugin:{plugin_id}");
    if let Some(existing) = providers.get_arc(&id) {
        return Ok(existing);
    }
    providers.register(
        ProviderDescriptor {
            id: id.clone(),
            kind: ProviderKind::Preview,
            priority: 50,
            r#match: ProviderMatchRule::default(),
        },
        Arc::new(IpcPreview {
            plugin_id: plugin_id.into(),
        }),
    )?;
    providers
        .get_arc(&id)
        .ok_or_else(|| "PreviewProvider registration failed".into())
}

pub(super) struct ScopedPreviewProxy {
    handler: Arc<dyn ScopedPreview>,
    plugin: PluginInfo,
    resource: Arc<ResourceSnapshot>,
}
impl ScopedPreviewProxy {
    pub fn new(plugin: PluginInfo, resource: Arc<ResourceSnapshot>) -> Result<Self, String> {
        Ok(Self {
            handler: proxy(&plugin.id)?,
            plugin,
            resource,
        })
    }

    pub fn open(&self, request: &ProviderRequest) -> Result<Value, String> {
        self.handler.open(&self.plugin, &self.resource, request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persistent_preview_proxy_stores_identity_only() {
        let one = proxy("org.example.preview").unwrap();
        let two = proxy("org.example.preview").unwrap();
        assert!(Arc::ptr_eq(&one, &two));
        assert!(proxy("").is_err());
        assert!(proxy("invalid/plugin").is_err());
    }
}
