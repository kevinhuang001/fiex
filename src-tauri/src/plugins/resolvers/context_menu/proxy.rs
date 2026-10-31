//! Persistent ContextMenu IPC identity; the operation's trusted selection,
//! directory and menu session belong to the short-lived request endpoint.
//! No session tokens, resource scopes or permissions are cached across calls.
use crate::plugins::{host::PluginInfo, resource::resource_context::ResourceSnapshot};
use fiex_provider_contracts::{
    ProviderDescriptor, ProviderInstances, ProviderKind, ProviderMatchRule, ProviderRequest,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex, OnceLock};

trait ScopedContextMenu: Send + Sync {
    fn invoke(
        &self,
        plugin: &PluginInfo,
        directory: &str,
        resources: &[ResourceSnapshot],
        request: &ProviderRequest,
    ) -> Result<Value, String>;
}

struct IpcContextMenu {
    plugin_id: String,
}
impl ScopedContextMenu for IpcContextMenu {
    fn invoke(
        &self,
        plugin: &PluginInfo,
        directory: &str,
        resources: &[ResourceSnapshot],
        request: &ProviderRequest,
    ) -> Result<Value, String> {
        if self.plugin_id != plugin.id {
            return Err("ContextMenuProvider plugin identity mismatch".into());
        }
        match request.method.as_str() {
            "contextMenu.resolve" => {
                let session_id = crate::plugins::mounts::new_id()?;
                let mut params = request.params.clone();
                let object = params.as_object_mut().ok_or("invalid menu parameters")?;
                object.insert("menuSessionId".into(), json!(session_id));
                object.insert(
                    "interactionId".into(),
                    json!(crate::plugins::mounts::new_id()?),
                );
                let mut result = crate::plugins::events::invoke_resources(
                    plugin,
                    "contextMenu.resolve",
                    params,
                    resources,
                    Some(directory),
                )?;
                result["menuSessionId"] = json!(session_id);
                Ok(result)
            }
            "contextMenu.execute" => {
                let mut result = crate::plugins::events::call_owner(
                    "contextMenu.execute",
                    &plugin.id,
                    request.params.clone(),
                    resources,
                    Some(directory),
                )?;
                if matches!(result["kind"].as_str(), Some("coreAction" | "remoteAction")) {
                    return Err("plugin returned a reserved action kind".into());
                }
                if result["kind"] == "panel" {
                    let entry = result["entry"]
                        .as_str()
                        .ok_or("panel result missing entry")?;
                    let asset = crate::plugins::host::plugin_view_asset(&plugin.id, entry)?;
                    let object = result.as_object_mut().ok_or("invalid panel result")?;
                    object.remove("entry");
                    object.insert("assetPath".into(), json!(asset));
                }
                Ok(result)
            }
            _ => Err("unsupported ContextMenu event".into()),
        }
    }
}

fn proxy(plugin_id: &str) -> Result<Arc<dyn ScopedContextMenu>, String> {
    if plugin_id.is_empty() {
        return Err("missing ContextMenuProvider plugin identity".into());
    }
    static REGISTERED: OnceLock<Mutex<ProviderInstances<dyn ScopedContextMenu>>> = OnceLock::new();
    let registry =
        REGISTERED.get_or_init(|| Mutex::new(ProviderInstances::new(ProviderKind::ContextMenu)));
    let mut providers = registry
        .lock()
        .map_err(|_| "ContextMenuProvider registry unavailable".to_string())?;
    let id = format!("plugin:{plugin_id}");
    if let Some(existing) = providers.get_arc(&id) {
        return Ok(existing);
    }
    providers.register(
        ProviderDescriptor {
            id: id.clone(),
            kind: ProviderKind::ContextMenu,
            priority: 50,
            r#match: ProviderMatchRule::default(),
        },
        Arc::new(IpcContextMenu {
            plugin_id: plugin_id.into(),
        }),
    )?;
    providers
        .get_arc(&id)
        .ok_or_else(|| "ContextMenuProvider registration failed".into())
}

pub(super) struct ScopedMenuProxy {
    handler: Arc<dyn ScopedContextMenu>,
    plugin: PluginInfo,
    directory: String,
    resources: Arc<Vec<ResourceSnapshot>>,
}
impl ScopedMenuProxy {
    pub fn new(
        plugin: PluginInfo,
        directory: String,
        resources: Arc<Vec<ResourceSnapshot>>,
    ) -> Result<Self, String> {
        Ok(Self {
            handler: proxy(&plugin.id)?,
            plugin,
            directory,
            resources,
        })
    }

    pub fn call(&self, request: &ProviderRequest) -> Result<Value, String> {
        self.handler
            .invoke(&self.plugin, &self.directory, &self.resources, request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persistent_menu_proxy_does_not_reuse_operation_state() {
        let a = proxy("org.example.menu").unwrap();
        let b = proxy("org.example.menu").unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        assert!(proxy("").is_err());
        assert!(proxy("invalid/menu").is_err());
    }
}
