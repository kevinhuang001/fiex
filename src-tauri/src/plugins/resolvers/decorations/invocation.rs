//! Core-owned per-request data for the shared DecorationProvider contract.
//! Trusted paths and rich native metadata stay behind the process-local handle.
use super::types::DecorationEntry;
use crate::plugins::{
    host::PluginInfo, native_invocation::NativeProviderInvocation,
    resource::resource_context::ResourceSnapshot,
};
use fiex_provider_contracts::{ProviderInvocation, ProviderRequestContext};
use std::{any::Any, collections::HashMap, sync::Arc};

pub(crate) struct DecorationServices {
    pub directory: String,
    pub entries: Arc<HashMap<String, DecorationEntry>>,
    pub resources: Arc<Vec<ResourceSnapshot>>,
    pub plugin: Option<PluginInfo>,
}

pub(crate) struct CoreDecorationInvocation {
    base: NativeProviderInvocation,
    services: DecorationServices,
}

impl CoreDecorationInvocation {
    pub fn native(
        directory: String,
        entries: Arc<HashMap<String, DecorationEntry>>,
        resources: Arc<Vec<ResourceSnapshot>>,
    ) -> Self {
        Self::new(directory, entries, resources, None, None)
    }

    pub fn plugin(
        directory: String,
        entries: Arc<HashMap<String, DecorationEntry>>,
        resources: Arc<Vec<ResourceSnapshot>>,
        plugin: PluginInfo,
        request: ProviderRequestContext,
    ) -> Self {
        Self::new(directory, entries, resources, Some(plugin), Some(request))
    }

    fn new(
        directory: String,
        entries: Arc<HashMap<String, DecorationEntry>>,
        resources: Arc<Vec<ResourceSnapshot>>,
        plugin: Option<PluginInfo>,
        request: Option<ProviderRequestContext>,
    ) -> Self {
        let directory_resource = ResourceSnapshot::from_metadata(
            &directory,
            crate::plugins::contracts::ResourceMetadata {
                kind: "directory".into(),
                ..Default::default()
            },
        );
        let base = request.map_or_else(
            || {
                NativeProviderInvocation::new(
                    "decorations.resolve",
                    serde_json::json!({}),
                    resources.as_slice(),
                    Some(&directory_resource),
                )
            },
            |request| {
                NativeProviderInvocation::from_request_context(
                    request,
                    resources.as_slice(),
                    Some(&directory_resource),
                )
            },
        );
        Self {
            base,
            services: DecorationServices {
                directory,
                entries,
                resources,
                plugin,
            },
        }
    }
}

impl ProviderInvocation for CoreDecorationInvocation {
    fn request(&self) -> &ProviderRequestContext {
        self.base.request()
    }

    fn local_service_handle(&self) -> Option<&dyn Any> {
        Some(&self.services)
    }
}

pub(crate) fn services(ctx: &dyn ProviderInvocation) -> Result<&DecorationServices, String> {
    ctx.local_service_handle()
        .and_then(|value| value.downcast_ref::<DecorationServices>())
        .ok_or_else(|| "DecorationProvider requires a Core-scoped invocation".into())
}
