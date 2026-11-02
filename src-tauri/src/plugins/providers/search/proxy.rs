//! Persistent, shared-trait SearchProvider IPC identity. Each invocation
//! supplies fresh Core-verified resources; cursors and cancellation remain
//! request-scoped in the streaming resolver.
use crate::plugins::{
    host::PluginInfo, native_invocation::NativeProviderInvocation,
    resource::resource_context::ResourceSnapshot,
};
use fiex_provider_contracts::{
    ProviderDescriptor, ProviderError, ProviderInput, ProviderInstances, ProviderInvocation,
    ProviderKind, ProviderMatchRule, SearchProvider, SearchResults,
};
use std::{
    any::Any,
    sync::{Arc, Mutex, OnceLock},
};

struct ScopedSearchInvocation {
    base: NativeProviderInvocation,
    plugin: PluginInfo,
    resources: Vec<ResourceSnapshot>,
    directory: String,
}

impl ProviderInvocation for ScopedSearchInvocation {
    fn request(&self) -> &fiex_provider_contracts::ProviderRequestContext {
        self.base.request()
    }

    fn local_service_handle(&self) -> Option<&dyn Any> {
        Some(self)
    }
}

struct IpcSearch {
    plugin_id: String,
}

impl SearchProvider for IpcSearch {
    fn query(&self, ctx: &dyn ProviderInvocation) -> Result<SearchResults, ProviderError> {
        let invocation = ctx
            .local_service_handle()
            .and_then(|value| value.downcast_ref::<ScopedSearchInvocation>())
            .ok_or_else(|| ProviderError::invalid_params("missing scoped search context"))?;
        if self.plugin_id != invocation.plugin.id || ctx.method() != "search.query" {
            return Err(ProviderError::invalid_params(
                "SearchProvider identity or method mismatch",
            ));
        }
        let response = crate::plugins::events::invoke_resources(
            &invocation.plugin,
            "search.query",
            ctx.input()?
                .to_params()
                .map_err(ProviderError::invalid_params)?,
            &invocation.resources,
            Some(&invocation.directory),
        )
        .map_err(ProviderError::internal)?;
        let page: SearchResults = serde_json::from_value(response)
            .map_err(|error| ProviderError::invalid_params(error.to_string()))?;
        let limit = match ctx.input()? {
            ProviderInput::Search(request) => request.bounded_limit(),
            _ => return Err(ProviderError::invalid_params("missing search request")),
        };
        page.validate_page(limit)
            .map_err(ProviderError::invalid_params)?;
        Ok(page)
    }
}

fn registry() -> &'static Mutex<ProviderInstances<dyn SearchProvider>> {
    static REGISTERED: OnceLock<Mutex<ProviderInstances<dyn SearchProvider>>> = OnceLock::new();
    REGISTERED.get_or_init(|| Mutex::new(ProviderInstances::new(ProviderKind::Search)))
}

pub(super) fn forget_plugin(plugin_id: &str) {
    if let Ok(mut providers) = registry().lock() {
        providers.unregister(&format!("plugin:{plugin_id}"));
    }
}

fn proxy(plugin_id: &str) -> Result<Arc<dyn SearchProvider>, String> {
    if plugin_id.is_empty() {
        return Err("missing SearchProvider plugin identity".into());
    }
    let mut registry = registry()
        .lock()
        .map_err(|_| "SearchProvider registry unavailable".to_string())?;
    let id = format!("plugin:{plugin_id}");
    if let Some(registered) = registry.get_arc(&id) {
        return Ok(registered);
    }
    registry.register(
        ProviderDescriptor {
            id: id.clone(),
            kind: ProviderKind::Search,
            priority: 50,
            r#match: ProviderMatchRule::default(),
        },
        Arc::new(IpcSearch {
            plugin_id: plugin_id.into(),
        }),
    )?;
    registry
        .get_arc(&id)
        .ok_or_else(|| "SearchProvider registration failed".into())
}

pub(super) struct ScopedSearchProxy {
    handler: Arc<dyn SearchProvider>,
    plugin: PluginInfo,
    resources: Vec<ResourceSnapshot>,
    directory: String,
    provider_id: String,
}

impl ScopedSearchProxy {
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

    fn invocation(
        &self,
        ctx: &dyn ProviderInvocation,
    ) -> Result<ScopedSearchInvocation, ProviderError> {
        let request = crate::plugins::events::bind_provider_context(
            &self.plugin.id,
            &self.provider_id,
            ctx.request(),
        )
        .map_err(ProviderError::invalid_params)?;
        Ok(ScopedSearchInvocation {
            base: NativeProviderInvocation::from_request_context(
                request,
                self.resources.as_slice(),
                None,
            ),
            plugin: self.plugin.clone(),
            resources: self.resources.clone(),
            directory: self.directory.clone(),
        })
    }
}

impl SearchProvider for ScopedSearchProxy {
    fn query(&self, ctx: &dyn ProviderInvocation) -> Result<SearchResults, ProviderError> {
        let mut entries = Vec::new();
        let page = self.query_stream(ctx, &|| false, &mut |entry| {
            entries.push(entry);
            Ok(())
        })?;
        Ok(SearchResults { entries, ..page })
    }

    fn query_stream(
        &self,
        ctx: &dyn ProviderInvocation,
        cancelled: &dyn Fn() -> bool,
        emit: &mut dyn FnMut(fiex_provider_contracts::SearchEntry) -> Result<(), ProviderError>,
    ) -> Result<SearchResults, ProviderError> {
        let invocation = self.invocation(ctx)?;
        SearchProvider::query_stream(self.handler.as_ref(), &invocation, cancelled, emit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_proxy_reuse_never_retains_pagination_state() {
        let a = proxy("org.example.search").unwrap();
        let b = proxy("org.example.search").unwrap();
        assert!(Arc::ptr_eq(&a, &b));
        assert!(proxy("").is_err());
        assert!(proxy("forged/provider").is_err());
    }
}
