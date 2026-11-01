//! A Toolbar proxy persists the plugin identity, not the current selection,
//! operation scope, or installation permissions. Scoped requests are fresh.
use crate::plugins::{
    host::PluginInfo,
    native_invocation::NativeProviderInvocation,
    resource::resource_context::ResourceSnapshot,
};
use fiex_provider_contracts::{
    ProviderDescriptor, ProviderError, ProviderInstances, ProviderInvocation, ProviderKind,
    ProviderMatchRule, ProviderRequest, ProviderRequestContext, ToolbarProvider,
};
use serde_json::Value;
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
        request: &ProviderRequest,
    ) -> Self {
        let directory_resource = ResourceSnapshot::from_metadata(
            directory,
            crate::plugins::contracts::ResourceMetadata {
                kind: "directory".into(),
                ..Default::default()
            },
        );
        Self {
            base: NativeProviderInvocation::new(
                request.method.clone(),
                request.params.clone(),
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
    fn invoke(&self, ctx: &dyn ProviderInvocation) -> Result<Value, ProviderError> {
        let services = ctx
            .local_service_handle()
            .and_then(|value| value.downcast_ref::<ToolbarServices>())
            .ok_or_else(|| ProviderError::internal("ToolbarProvider requires a Core invocation"))?;
        if services.plugin.id != self.plugin_id || ctx.method() != "toolbar.invoke" {
            return Err(ProviderError::invalid_params(
                "ToolbarProvider identity or method mismatch",
            ));
        }
        crate::plugins::events::invoke_resources(
            &services.plugin,
            ctx.method(),
            ctx.params().clone(),
            &services.resources,
            Some(services.directory.as_str()),
        )
        .map_err(ProviderError::internal)
    }
}

fn proxy(plugin_id: &str) -> Result<Arc<dyn ToolbarProvider>, String> {
    if plugin_id.is_empty() {
        return Err("missing ToolbarProvider plugin identity".into());
    }
    static REGISTERED: OnceLock<Mutex<ProviderInstances<dyn ToolbarProvider>>> = OnceLock::new();
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

pub(super) fn invoke(
    plugin: &PluginInfo,
    resources: &[ResourceSnapshot],
    directory: &str,
    request: &ProviderRequest,
) -> Result<Value, String> {
    let invocation = CoreToolbarInvocation::new(plugin.clone(), resources, directory, request);
    ToolbarProvider::invoke(proxy(&plugin.id)?.as_ref(), &invocation)
        .map_err(|error| error.to_string())
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
