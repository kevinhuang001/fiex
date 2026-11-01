//! Persistent ContextMenu IPC identity; trusted selection, directory and
//! menu session belong to each short-lived invocation, never the registry.
use crate::plugins::{host::PluginInfo, resource::resource_context::ResourceSnapshot};
use fiex_provider_contracts::{
    ContextMenuProvider, ProviderDescriptor, ProviderError, ProviderInstances, ProviderInvocation,
    ProviderKind, ProviderMatchRule, ProviderRequest, ProviderRequestContext,
};
use serde_json::{json, Value};
use std::{
    any::Any,
    sync::{Arc, Mutex, OnceLock},
};

struct ScopedMenuInvocation {
    request: ProviderRequestContext,
    plugin: PluginInfo,
    directory: String,
    resources: Arc<Vec<ResourceSnapshot>>,
    remote_session_id: Option<String>,
}

impl ScopedMenuInvocation {
    fn new(
        plugin: PluginInfo,
        directory: String,
        resources: Arc<Vec<ResourceSnapshot>>,
        request: &ProviderRequest,
    ) -> Result<Self, String> {
        let mut params = request.params.clone();
        let remote_session_id =
            if request.method == "contextMenu.resolve" && params.get("parentId").is_none() {
                let session_id = crate::plugins::mounts::new_id()?;
                let object = params.as_object_mut().ok_or("invalid menu parameters")?;
                object.insert("menuSessionId".into(), json!(session_id));
                Some(session_id)
            } else {
                params
                    .get("menuSessionId")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            };
        let object = params.as_object_mut().ok_or("invalid menu parameters")?;
        object.insert(
            "interactionId".into(),
            json!(crate::plugins::mounts::new_id()?),
        );
        Ok(Self {
            request: ProviderRequestContext::from_params(
                request.method.clone(),
                params,
                crate::i18n::locale(),
            ),
            plugin,
            directory,
            resources,
            remote_session_id,
        })
    }
}

impl ProviderInvocation for ScopedMenuInvocation {
    fn request(&self) -> &ProviderRequestContext {
        &self.request
    }

    fn local_service_handle(&self) -> Option<&dyn Any> {
        Some(self)
    }
}

struct IpcContextMenu {
    plugin_id: String,
}

impl IpcContextMenu {
    fn invocation<'a>(
        &self,
        ctx: &'a dyn ProviderInvocation,
        method: &str,
    ) -> Result<&'a ScopedMenuInvocation, ProviderError> {
        let invocation = ctx
            .local_service_handle()
            .and_then(|value| value.downcast_ref::<ScopedMenuInvocation>())
            .ok_or_else(|| ProviderError::invalid_params("missing scoped menu context"))?;
        if self.plugin_id != invocation.plugin.id || ctx.method() != method {
            return Err(ProviderError::invalid_params(
                "ContextMenuProvider identity or method mismatch",
            ));
        }
        Ok(invocation)
    }
}

impl ContextMenuProvider for IpcContextMenu {
    fn resolve(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<Value>, ProviderError> {
        let invocation = self.invocation(ctx, "contextMenu.resolve")?;
        let result = crate::plugins::events::invoke_resources(
            &invocation.plugin,
            "contextMenu.resolve",
            ctx.params().clone(),
            &invocation.resources,
            Some(&invocation.directory),
        )
        .map_err(ProviderError::internal)?;
        serde_json::from_value(result["items"].clone())
            .map_err(|error| ProviderError::internal(error.to_string()))
    }

    fn execute(&self, ctx: &dyn ProviderInvocation) -> Result<Value, ProviderError> {
        let invocation = self.invocation(ctx, "contextMenu.execute")?;
        let mut result = crate::plugins::events::call_owner(
            "contextMenu.execute",
            &invocation.plugin.id,
            ctx.params().clone(),
            &invocation.resources,
            Some(&invocation.directory),
        )
        .map_err(ProviderError::internal)?;
        if matches!(result["kind"].as_str(), Some("coreAction" | "remoteAction")) {
            return Err(ProviderError::invalid_params(
                "plugin returned a reserved action kind",
            ));
        }
        if result["kind"] == "panel" {
            let entry = result["entry"]
                .as_str()
                .ok_or_else(|| ProviderError::invalid_params("panel result missing entry"))?;
            let asset = crate::plugins::host::plugin_view_asset(&invocation.plugin.id, entry)
                .map_err(ProviderError::internal)?;
            let object = result
                .as_object_mut()
                .ok_or_else(|| ProviderError::invalid_params("invalid panel result"))?;
            object.remove("entry");
            object.insert("assetPath".into(), json!(asset));
        }
        Ok(result)
    }
}

fn registry() -> &'static Mutex<ProviderInstances<dyn ContextMenuProvider>> {
    static REGISTERED: OnceLock<Mutex<ProviderInstances<dyn ContextMenuProvider>>> =
        OnceLock::new();
    REGISTERED.get_or_init(|| Mutex::new(ProviderInstances::new(ProviderKind::ContextMenu)))
}

pub(super) fn forget_plugin(plugin_id: &str) {
    if let Ok(mut providers) = registry().lock() {
        providers.unregister(&format!("plugin:{plugin_id}"));
    }
}

fn proxy(plugin_id: &str) -> Result<Arc<dyn ContextMenuProvider>, String> {
    if plugin_id.is_empty() {
        return Err("missing ContextMenuProvider plugin identity".into());
    }
    let mut providers = registry()
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
    handler: Arc<dyn ContextMenuProvider>,
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

    pub fn resolve_typed(&self, request: &ProviderRequest) -> Result<Value, String> {
        let invocation = ScopedMenuInvocation::new(
            self.plugin.clone(),
            self.directory.clone(),
            self.resources.clone(),
            request,
        )?;
        let items = ContextMenuProvider::resolve(self.handler.as_ref(), &invocation)
            .map_err(|error| error.to_string())?;
        let session_id = invocation
            .remote_session_id
            .as_deref()
            .ok_or("plugin menu session ID is missing")?;
        Ok(json!({"items":items,"menuSessionId":session_id}))
    }

    pub fn execute_typed(&self, request: &ProviderRequest) -> Result<Value, String> {
        let invocation = ScopedMenuInvocation::new(
            self.plugin.clone(),
            self.directory.clone(),
            self.resources.clone(),
            request,
        )?;
        ContextMenuProvider::execute(self.handler.as_ref(), &invocation)
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_menu_proxies_reuse_identity_without_retaining_scope() {
        let a = proxy("org.example.menu").unwrap();
        let b = proxy("org.example.menu").unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        assert!(proxy("").is_err());
        assert!(proxy("forged/provider").is_err());
    }
}
