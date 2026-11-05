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
}

impl CoreDecorationInvocation {
    fn services(
        directory: String,
        entries: Arc<HashMap<String, DecorationEntry>>,
        resources: Arc<Vec<ResourceSnapshot>>,
        plugin: Option<PluginInfo>,
    ) -> Arc<DecorationServices> {
        Arc::new(DecorationServices {
            directory,
            entries,
            resources,
            plugin,
        })
    }

    pub fn native(
        provider_id: &str,
        directory: String,
        entries: Arc<HashMap<String, DecorationEntry>>,
        resources: Arc<Vec<ResourceSnapshot>>,
    ) -> Result<Self, String> {
        let directory_resource = crate::plugins::resource::resource_context::from_metadata(
            &directory,
            crate::plugins::contracts::ResourceMetadata {
                kind: "directory".into(),
                ..Default::default()
            },
        );
        let base = super::native::invocation(
            provider_id,
            "decorations.resolve",
            serde_json::json!({}),
            resources.as_slice(),
            Some(&directory_resource),
        )?
        .with_local_data(Self::services(directory, entries, resources, None));
        Ok(Self { base })
    }

    pub fn unbound(
        directory: String,
        entries: Arc<HashMap<String, DecorationEntry>>,
        resources: Arc<Vec<ResourceSnapshot>>,
    ) -> Self {
        let directory_resource = crate::plugins::resource::resource_context::from_metadata(
            &directory,
            crate::plugins::contracts::ResourceMetadata {
                kind: "directory".into(),
                ..Default::default()
            },
        );
        let base = NativeProviderInvocation::new(
            "decorations.resolve",
            serde_json::json!({}),
            resources.as_slice(),
            Some(&directory_resource),
        )
        .with_local_data(Self::services(directory, entries, resources, None));
        Self { base }
    }

    pub fn plugin(
        directory: String,
        entries: Arc<HashMap<String, DecorationEntry>>,
        resources: Arc<Vec<ResourceSnapshot>>,
        plugin: PluginInfo,
        request: ProviderRequestContext,
    ) -> Self {
        let directory_resource = crate::plugins::resource::resource_context::from_metadata(
            &directory,
            crate::plugins::contracts::ResourceMetadata {
                kind: "directory".into(),
                ..Default::default()
            },
        );
        let base = NativeProviderInvocation::from_request_context(
            request,
            resources.as_slice(),
            Some(&directory_resource),
        )
        .with_local_data(Self::services(directory, entries, resources, Some(plugin)));
        Self { base }
    }
}

impl ProviderInvocation for CoreDecorationInvocation {
    fn request(&self) -> &ProviderRequestContext {
        self.base.request()
    }

    fn local_service_handle(&self) -> Option<&dyn Any> {
        Some(&self.base)
    }
}

pub(crate) fn services(ctx: &dyn ProviderInvocation) -> Result<&DecorationServices, String> {
    NativeProviderInvocation::local_data(ctx).map_err(|error| error.to_string())
}
