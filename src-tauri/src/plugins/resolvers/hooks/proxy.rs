//! The Hook IPC proxy is persistent, but each invocation receives the
//! current trusted plugin record and an operation-scoped resource snapshot.
//! In particular, no mount, selection, permission grant or path is cached.
use crate::plugins::{
    host::PluginInfo, native_invocation::NativeProviderInvocation,
    resource::resource_context::ResourceSnapshot,
};
use fiex_provider_contracts::{
    HookProvider, HookResult, ProviderDescriptor, ProviderError, ProviderInstances,
    ProviderInvocation, ProviderKind, ProviderMatchRule, ProviderRequest, ProviderRequestContext,
};
use std::{
    any::Any,
    sync::{Arc, Mutex, OnceLock},
};

struct HookServices {
    plugin: PluginInfo,
    resource: ResourceSnapshot,
}

struct CoreHookInvocation {
    base: NativeProviderInvocation,
    services: HookServices,
}

impl CoreHookInvocation {
    fn new(plugin: PluginInfo, resource: &ResourceSnapshot, request: &ProviderRequest) -> Self {
        Self {
            base: NativeProviderInvocation::new(
                request.method.clone(),
                request.params.clone(),
                std::slice::from_ref(resource),
                None,
            ),
            services: HookServices {
                plugin,
                resource: resource.clone(),
            },
        }
    }
}

impl ProviderInvocation for CoreHookInvocation {
    fn request(&self) -> &ProviderRequestContext {
        self.base.request()
    }

    fn local_service_handle(&self) -> Option<&dyn Any> {
        Some(&self.services)
    }
}

struct IpcHook {
    plugin_id: String,
}

impl HookProvider for IpcHook {
    fn invoke(&self, ctx: &dyn ProviderInvocation) -> Result<HookResult, ProviderError> {
        let services = ctx
            .local_service_handle()
            .and_then(|value| value.downcast_ref::<HookServices>())
            .ok_or_else(|| ProviderError::internal("HookProvider requires a Core invocation"))?;
        if services.plugin.id != self.plugin_id || ctx.method() != "hooks.invoke" {
            return Err(ProviderError::invalid_params(
                "HookProvider identity or event mismatch",
            ));
        }
        let value = crate::plugins::events::invoke_resources(
            &services.plugin,
            ctx.method(),
            ctx.params().clone(),
            std::slice::from_ref(&services.resource),
            None,
        )
        .map_err(ProviderError::internal)?;
        serde_json::from_value(value)
            .map_err(|error| ProviderError::invalid_params(error.to_string()))
    }
}

fn registry() -> &'static Mutex<ProviderInstances<dyn HookProvider>> {
    static REGISTERED: OnceLock<Mutex<ProviderInstances<dyn HookProvider>>> = OnceLock::new();
    REGISTERED.get_or_init(|| Mutex::new(ProviderInstances::new(ProviderKind::Hook)))
}

pub(super) fn forget_plugin(plugin_id: &str) {
    if let Ok(mut providers) = registry().lock() {
        providers.unregister(&format!("plugin:{plugin_id}"));
    }
}

fn proxy(plugin_id: &str) -> Result<Arc<dyn HookProvider>, String> {
    if plugin_id.is_empty() {
        return Err("missing HookProvider plugin identity".into());
    }
    let mut providers = registry()
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

pub(super) fn invoke(
    plugin: &PluginInfo,
    resource: &ResourceSnapshot,
    request: &ProviderRequest,
) -> Result<HookResult, String> {
    let invocation = CoreHookInvocation::new(plugin.clone(), resource, request);
    HookProvider::invoke(proxy(&plugin.id)?.as_ref(), &invocation)
        .map_err(|error| error.to_string())
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
