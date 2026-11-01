//! Persistent ContextMenu IPC identity; trusted selection, directory and
//! menu session belong to each short-lived invocation, never the registry.
use crate::plugins::{host::PluginInfo, resource::resource_context::ResourceSnapshot};
use fiex_provider_contracts::{
    ContextMenuProvider, MenuExecuteResult, MenuItem, ProviderDescriptor, ProviderError,
    ProviderInstances, ProviderInvocation, ProviderKind, ProviderMatchRule, ProviderRequestContext,
};
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
        request: &ProviderRequestContext,
    ) -> Result<Self, String> {
        let mut request = request.clone();
        let (root_resolve, existing_session) = match request
            .input()
            .map_err(|error| error.to_string())?
        {
            fiex_provider_contracts::ProviderInput::ContextMenuResolve(input) => {
                (input.parent_id.is_none(), input.menu_session_id.clone())
            }
            fiex_provider_contracts::ProviderInput::ContextMenuExecute(input) => {
                (false, input.menu_session_id.clone())
            }
            _ => return Err("invalid context menu request".into()),
        };
        let root_session = if root_resolve {
            Some(crate::plugins::mounts::new_id()?)
        } else {
            None
        };
        let remote_session_id = root_session.clone().or(existing_session);
        request
            .input
            .set_context_menu_interaction(root_session, crate::plugins::mounts::new_id()?)?;
        request.input_error = None;
        Ok(Self {
            request,
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
    fn resolve(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<MenuItem>, ProviderError> {
        let invocation = self.invocation(ctx, "contextMenu.resolve")?;
        let result = crate::plugins::events::invoke_resources(
            &invocation.plugin,
            "contextMenu.resolve",
            ctx.input()?.to_params().map_err(ProviderError::invalid_params)?,
            &invocation.resources,
            Some(&invocation.directory),
        )
        .map_err(ProviderError::internal)?;
        serde_json::from_value(result["items"].clone())
            .map_err(|error| ProviderError::internal(error.to_string()))
    }

    fn execute(&self, ctx: &dyn ProviderInvocation) -> Result<MenuExecuteResult, ProviderError> {
        let invocation = self.invocation(ctx, "contextMenu.execute")?;
        let result = crate::plugins::events::invoke_resources(
            &invocation.plugin,
            "contextMenu.execute",
            ctx.input()?.to_params().map_err(ProviderError::invalid_params)?,
            &invocation.resources,
            Some(&invocation.directory),
        )
        .map_err(ProviderError::internal)?;
        serde_json::from_value(result)
            .map_err(|error| ProviderError::invalid_params(error.to_string()))
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
    provider_id: String,
    directory: String,
    resources: Arc<Vec<ResourceSnapshot>>,
    remote_session_id: Arc<Mutex<Option<String>>>,
}

impl ScopedMenuProxy {
    pub fn new(
        plugin: PluginInfo,
        provider_id: String,
        directory: String,
        resources: Arc<Vec<ResourceSnapshot>>,
    ) -> Result<Self, String> {
        Ok(Self {
            handler: proxy(&plugin.id)?,
            plugin,
            provider_id,
            directory,
            resources,
            remote_session_id: Arc::new(Mutex::new(None)),
        })
    }

    pub fn remote_session_slot(&self) -> Arc<Mutex<Option<String>>> {
        Arc::clone(&self.remote_session_id)
    }
}

impl ContextMenuProvider for ScopedMenuProxy {
    fn resolve(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<MenuItem>, ProviderError> {
        let request = crate::plugins::events::bind_provider_context(
            &self.plugin.id,
            &self.provider_id,
            ctx.request(),
        )
        .map_err(ProviderError::invalid_params)?;
        let invocation = ScopedMenuInvocation::new(
            self.plugin.clone(),
            self.directory.clone(),
            self.resources.clone(),
            &request,
        )
        .map_err(ProviderError::internal)?;
        let items = self.handler.resolve(&invocation)?;
        if let Ok(mut current) = self.remote_session_id.lock() {
            *current = invocation.remote_session_id.clone();
        }
        Ok(items)
    }

    fn execute(&self, ctx: &dyn ProviderInvocation) -> Result<MenuExecuteResult, ProviderError> {
        let request = crate::plugins::events::bind_provider_context(
            &self.plugin.id,
            &self.provider_id,
            ctx.request(),
        )
        .map_err(ProviderError::invalid_params)?;
        let invocation = ScopedMenuInvocation::new(
            self.plugin.clone(),
            self.directory.clone(),
            self.resources.clone(),
            &request,
        )
        .map_err(ProviderError::internal)?;
        self.handler.execute(&invocation)
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
