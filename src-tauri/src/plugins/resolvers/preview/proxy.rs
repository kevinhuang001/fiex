//! Persistent PreviewProvider IPC identity with invocation-scoped resources.
use super::session::{open_plugin_resource, UNSUPPORTED};
use crate::plugins::{
    host::PluginInfo, resource::resource_context::ResourceSnapshot,
};
use fiex_provider_contracts::{
    PreviewProvider, PreviewView, ProviderDescriptor, ProviderError, ProviderInstances,
    ProviderInput, ProviderInvocation, ProviderKind, ProviderMatchRule, ProviderRequestContext,
};
use std::{
    any::Any,
    sync::{Arc, Mutex, OnceLock},
};

struct PreviewServices {
    plugin: PluginInfo,
    resource: ResourceSnapshot,
}

struct CorePreviewInvocation {
    request: ProviderRequestContext,
    services: PreviewServices,
}

impl CorePreviewInvocation {
    fn new(
        ctx: &dyn ProviderInvocation,
        plugin: PluginInfo,
        resource: ResourceSnapshot,
        registered_id: &str,
    ) -> Result<Self, ProviderError> {
        let input = crate::plugins::events::bind_provider_input(
            &plugin.id,
            registered_id,
            ctx.input()?,
        )
        .map_err(ProviderError::invalid_params)?;
        let mut request = ctx.request().clone();
        request.input = input;
        request.input_error = None;
        Ok(Self {
            request,
            services: PreviewServices { plugin, resource },
        })
    }
}

impl ProviderInvocation for CorePreviewInvocation {
    fn request(&self) -> &ProviderRequestContext {
        &self.request
    }

    fn local_service_handle(&self) -> Option<&dyn Any> {
        Some(&self.services)
    }
}

struct IpcPreview {
    plugin_id: String,
}

impl PreviewProvider for IpcPreview {
    fn open(&self, ctx: &dyn ProviderInvocation) -> Result<PreviewView, ProviderError> {
        let services = ctx
            .local_service_handle()
            .and_then(|value| value.downcast_ref::<PreviewServices>())
            .ok_or_else(|| ProviderError::internal("PreviewProvider requires a Core invocation"))?;
        if services.plugin.id != self.plugin_id || ctx.method() != "preview.open" {
            return Err(ProviderError::invalid_params(
                "PreviewProvider identity or event mismatch",
            ));
        }
        let provider_id = match ctx.input()? {
            ProviderInput::Resource(request) => request.provider_id.as_deref(),
            _ => None,
        };
        let response = match open_plugin_resource(&services.resource, &services.plugin, provider_id) {
            Ok(response) => response,
            Err(error) if error == UNSUPPORTED => {
                return Err(ProviderError::invalid_params(UNSUPPORTED));
            }
            Err(error) => return Err(ProviderError::internal(error)),
        };
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
        serde_json::from_value(view).map_err(|error| {
            ProviderError::invalid_params(format!("invalid PreviewView: {error}"))
        })
    }
}

fn registry() -> &'static Mutex<ProviderInstances<dyn PreviewProvider>> {
    static REGISTERED: OnceLock<Mutex<ProviderInstances<dyn PreviewProvider>>> = OnceLock::new();
    REGISTERED.get_or_init(|| Mutex::new(ProviderInstances::new(ProviderKind::Preview)))
}

pub(super) fn forget_plugin(plugin_id: &str) {
    if let Ok(mut providers) = registry().lock() {
        providers.unregister(&format!("plugin:{plugin_id}"));
    }
}

fn proxy(plugin_id: &str) -> Result<Arc<dyn PreviewProvider>, String> {
    if plugin_id.is_empty() {
        return Err("missing PreviewProvider plugin identity".into());
    }
    let mut providers = registry()
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
    handler: Arc<dyn PreviewProvider>,
    plugin: PluginInfo,
    provider_id: String,
    resource: Arc<ResourceSnapshot>,
}

impl ScopedPreviewProxy {
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

impl PreviewProvider for ScopedPreviewProxy {
    fn open(&self, ctx: &dyn ProviderInvocation) -> Result<PreviewView, ProviderError> {
        if ctx.method() != "preview.open" {
            return Err(ProviderError::invalid_params(
                "PreviewProvider method mismatch",
            ));
        }
        let invocation = CorePreviewInvocation::new(
            ctx,
            self.plugin.clone(),
            self.resource.as_ref().clone(),
            &self.provider_id,
        )?;
        self.handler.open(&invocation)
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
