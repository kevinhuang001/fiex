//! Persistent InfoProvider IPC identity with invocation-scoped resources.
use crate::plugins::{host::PluginInfo, resource::resource_context::ResourceSnapshot};
use fiex_provider_contracts::{
    InfoProvider, InfoSection, ProviderDescriptor, ProviderError, ProviderInstances,
    ProviderInvocation, ProviderKind, ProviderMatchRule, ProviderRequestContext,
};
use std::{
    any::Any,
    sync::{Arc, Mutex, OnceLock},
};

struct InfoServices {
    plugin: PluginInfo,
    resource: ResourceSnapshot,
}

struct CoreInfoInvocation {
    request: ProviderRequestContext,
    services: InfoServices,
}

impl CoreInfoInvocation {
    fn new(
        ctx: &dyn ProviderInvocation,
        plugin: PluginInfo,
        resource: ResourceSnapshot,
        registered_id: &str,
    ) -> Result<Self, ProviderError> {
        let input =
            crate::plugins::events::bind_provider_input(&plugin.id, registered_id, ctx.input()?)
                .map_err(ProviderError::invalid_params)?;
        let mut request = ctx.request().clone();
        request.input = input;
        request.input_error = None;
        Ok(Self {
            request,
            services: InfoServices { plugin, resource },
        })
    }
}

impl ProviderInvocation for CoreInfoInvocation {
    fn request(&self) -> &ProviderRequestContext {
        &self.request
    }

    fn local_service_handle(&self) -> Option<&dyn Any> {
        Some(&self.services)
    }
}

struct IpcInfo {
    plugin_id: String,
}

impl InfoProvider for IpcInfo {
    fn resolve(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<InfoSection>, ProviderError> {
        let services = ctx
            .local_service_handle()
            .and_then(|value| value.downcast_ref::<InfoServices>())
            .ok_or_else(|| ProviderError::internal("InfoProvider requires a Core invocation"))?;
        if services.plugin.id != self.plugin_id || ctx.method() != "info.resolve" {
            return Err(ProviderError::invalid_params(
                "InfoProvider identity or event mismatch",
            ));
        }
        let response = crate::plugins::events::invoke_resources(
            &services.plugin,
            ctx.method(),
            ctx.input()?
                .to_params()
                .map_err(ProviderError::invalid_params)?,
            std::slice::from_ref(&services.resource),
            None,
        )
        .map_err(ProviderError::internal)?;
        serde_json::from_value(response["sections"].clone())
            .map_err(|error| ProviderError::invalid_params(error.to_string()))
    }
}

fn registry() -> &'static Mutex<ProviderInstances<dyn InfoProvider>> {
    static REGISTERED: OnceLock<Mutex<ProviderInstances<dyn InfoProvider>>> = OnceLock::new();
    REGISTERED.get_or_init(|| Mutex::new(ProviderInstances::new(ProviderKind::Info)))
}

pub(super) fn forget_plugin(plugin_id: &str) {
    if let Ok(mut providers) = registry().lock() {
        providers.unregister(&format!("plugin:{plugin_id}"));
    }
}

fn proxy(plugin_id: &str) -> Result<Arc<dyn InfoProvider>, String> {
    if plugin_id.is_empty() {
        return Err("missing InfoProvider plugin identity".into());
    }
    let mut providers = registry()
        .lock()
        .map_err(|_| "InfoProvider registry unavailable".to_string())?;
    let id = format!("plugin:{plugin_id}");
    if let Some(instance) = providers.get_arc(&id) {
        return Ok(instance);
    }
    providers.register(
        ProviderDescriptor {
            id: id.clone(),
            kind: ProviderKind::Info,
            priority: 50,
            r#match: ProviderMatchRule::default(),
        },
        Arc::new(IpcInfo {
            plugin_id: plugin_id.into(),
        }),
    )?;
    providers
        .get_arc(&id)
        .ok_or_else(|| "InfoProvider registration failed".into())
}

pub(super) struct ScopedInfoProxy {
    handler: Arc<dyn InfoProvider>,
    plugin: PluginInfo,
    provider_id: String,
    resource: Arc<ResourceSnapshot>,
}

impl ScopedInfoProxy {
    pub fn new(
        plugin: PluginInfo,
        resource: Arc<ResourceSnapshot>,
        provider_id: String,
    ) -> Result<Self, String> {
        Ok(Self {
            handler: proxy(&plugin.id)?,
            plugin,
            provider_id,
            resource,
        })
    }
}

impl InfoProvider for ScopedInfoProxy {
    fn resolve(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<InfoSection>, ProviderError> {
        if ctx.method() != "info.resolve" {
            return Err(ProviderError::invalid_params(
                "InfoProvider method mismatch",
            ));
        }
        let invocation = CoreInfoInvocation::new(
            ctx,
            self.plugin.clone(),
            self.resource.as_ref().clone(),
            &self.provider_id,
        )?;
        self.handler.resolve(&invocation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_proxies_reuse_identities_without_retaining_resources_or_grants() {
        let a = proxy("org.example.info").unwrap();
        let b = proxy("org.example.info").unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        assert!(proxy("").is_err());
        assert!(proxy("bad/id").is_err());
    }
}
