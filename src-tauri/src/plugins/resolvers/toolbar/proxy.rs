//! A Toolbar proxy persists the plugin identity, not the current selection,
//! operation scope, or installation permissions. Scoped requests are fresh.
use crate::plugins::{
    host::PluginInfo, native_invocation::NativeProviderInvocation,
    resource::resource_context::ResourceSnapshot,
};
use fiex_provider_contracts::{
    ProviderDescriptor, ProviderError, ProviderInstances, ProviderInvocation, ProviderKind,
    ProviderMatchRule, ProviderRequestContext, ToolbarProvider, ToolbarResult,
};
use std::{
    any::Any,
    sync::{Arc, Mutex, OnceLock},
};

struct ToolbarServices {
    plugin: PluginInfo,
    resources: Vec<ResourceSnapshot>,
    directory: String,
}

struct CoreToolbarInvocation {
    base: NativeProviderInvocation,
    services: ToolbarServices,
}

impl CoreToolbarInvocation {
    fn new(
        plugin: PluginInfo,
        resources: &[ResourceSnapshot],
        directory: &str,
        request: ProviderRequestContext,
    ) -> Self {
        let directory_resource = ResourceSnapshot::from_metadata(
            directory,
            crate::plugins::contracts::ResourceMetadata {
                kind: "directory".into(),
                ..Default::default()
            },
        );
        Self {
            base: NativeProviderInvocation::from_request_context(
                request,
                resources,
                Some(&directory_resource),
            ),
            services: ToolbarServices {
                plugin,
                resources: resources.to_vec(),
                directory: directory.into(),
            },
        }
    }
}

impl ProviderInvocation for CoreToolbarInvocation {
    fn request(&self) -> &ProviderRequestContext {
        self.base.request()
    }

    fn local_service_handle(&self) -> Option<&dyn Any> {
        Some(&self.services)
    }
}

struct IpcToolbar {
    plugin_id: String,
}

impl ToolbarProvider for IpcToolbar {
    fn invoke(&self, ctx: &dyn ProviderInvocation) -> Result<ToolbarResult, ProviderError> {
        let services = ctx
            .local_service_handle()
            .and_then(|value| value.downcast_ref::<ToolbarServices>())
            .ok_or_else(|| ProviderError::internal("ToolbarProvider requires a Core invocation"))?;
        if services.plugin.id != self.plugin_id || ctx.method() != "toolbar.invoke" {
            return Err(ProviderError::invalid_params(
                "ToolbarProvider identity or method mismatch",
            ));
        }
        let response = crate::plugins::events::invoke_resources(
            &services.plugin,
            ctx.method(),
            ctx.input()?.to_params().map_err(ProviderError::invalid_params)?,
            &services.resources,
            Some(services.directory.as_str()),
        )
        .map_err(ProviderError::internal)?;
        serde_json::from_value(response)
            .map_err(|error| ProviderError::invalid_params(error.to_string()))
    }
}

fn registry() -> &'static Mutex<ProviderInstances<dyn ToolbarProvider>> {
    static REGISTERED: OnceLock<Mutex<ProviderInstances<dyn ToolbarProvider>>> = OnceLock::new();
    REGISTERED.get_or_init(|| Mutex::new(ProviderInstances::new(ProviderKind::Toolbar)))
}

pub(super) fn forget_plugin(plugin_id: &str) {
    if let Ok(mut providers) = registry().lock() {
        providers.unregister(&format!("plugin:{plugin_id}"));
    }
}

fn proxy(plugin_id: &str) -> Result<Arc<dyn ToolbarProvider>, String> {
    if plugin_id.is_empty() {
        return Err("missing ToolbarProvider plugin identity".into());
    }
    let mut providers = registry()
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

pub(super) struct ScopedToolbarProxy {
    handler: Arc<dyn ToolbarProvider>,
    plugin: PluginInfo,
    resources: Vec<ResourceSnapshot>,
    directory: String,
    provider_id: String,
}

impl ScopedToolbarProxy {
    pub fn new(
        plugin: PluginInfo,
        resources: &[ResourceSnapshot],
        directory: &str,
        provider_id: String,
    ) -> Result<Self, String> {
        Ok(Self {
            handler: proxy(&plugin.id)?,
            plugin,
            resources: resources.to_vec(),
            directory: directory.into(),
            provider_id,
        })
    }
}

impl ToolbarProvider for ScopedToolbarProxy {
    fn invoke(&self, ctx: &dyn ProviderInvocation) -> Result<ToolbarResult, ProviderError> {
        let request = crate::plugins::events::bind_provider_context(
            &self.plugin.id,
            &self.provider_id,
            ctx.request(),
        )
        .map_err(ProviderError::invalid_params)?;
        let invocation = CoreToolbarInvocation::new(
            self.plugin.clone(),
            &self.resources,
            &self.directory,
            request,
        );
        self.handler.invoke(&invocation)
    }
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
