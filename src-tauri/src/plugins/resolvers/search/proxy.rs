//! Persistent scoped SearchProvider IPC identity. Each page is dispatched
//! with fresh Core-verified resources; cursors and cancellation live in the
//! streaming resolver rather than in this shared proxy.
use crate::plugins::{host::PluginInfo, resource::resource_context::ResourceSnapshot};
use fiex_provider_contracts::{
    ProviderDescriptor, ProviderInstances, ProviderKind, ProviderMatchRule, ProviderRequest,
};
use serde_json::Value;
use std::sync::{Arc, Mutex, OnceLock};

trait ScopedSearch: Send + Sync {
    fn query(
        &self,
        plugin: &PluginInfo,
        resources: &[ResourceSnapshot],
        directory: &str,
        request: &ProviderRequest,
    ) -> Result<Value, String>;
}

struct IpcSearch {
    plugin_id: String,
}
impl ScopedSearch for IpcSearch {
    fn query(
        &self,
        plugin: &PluginInfo,
        resources: &[ResourceSnapshot],
        directory: &str,
        request: &ProviderRequest,
    ) -> Result<Value, String> {
        if self.plugin_id != plugin.id || request.method != "search.query" {
            return Err("SearchProvider identity or method mismatch".into());
        }
        crate::plugins::events::invoke_resources(
            plugin,
            "search.query",
            request.params.clone(),
            resources,
            Some(directory),
        )
    }
}

fn proxy(plugin_id: &str) -> Result<Arc<dyn ScopedSearch>, String> {
    if plugin_id.is_empty() {
        return Err("missing SearchProvider plugin identity".into());
    }
    static REGISTERED: OnceLock<Mutex<ProviderInstances<dyn ScopedSearch>>> = OnceLock::new();
    let registry =
        REGISTERED.get_or_init(|| Mutex::new(ProviderInstances::new(ProviderKind::Search)));
    let mut registry = registry
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

pub(super) struct ScopedSearchProxy<'a> {
    handler: Arc<dyn ScopedSearch>,
    plugin: PluginInfo,
    resources: &'a [ResourceSnapshot],
    directory: &'a str,
}
impl<'a> ScopedSearchProxy<'a> {
    pub fn new(
        plugin: PluginInfo,
        resources: &'a [ResourceSnapshot],
        directory: &'a str,
    ) -> Result<Self, String> {
        Ok(Self {
            handler: proxy(&plugin.id)?,
            plugin,
            resources,
            directory,
        })
    }

    pub fn query(&self, request: &ProviderRequest) -> Result<Value, String> {
        self.handler
            .query(&self.plugin, self.resources, self.directory, request)
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
