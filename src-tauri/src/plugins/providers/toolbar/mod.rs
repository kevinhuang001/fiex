//! Toolbar commands share the canonical discovery → scoped invocation →
//! validation pipeline, with a persistent IPC proxy per plugin identity.
//! Only the current selection snapshot crosses the operation boundary.
mod proxy;

pub(crate) fn forget_plugin(plugin_id: &str) {
    proxy::forget_plugin(plugin_id);
}

use crate::plugins::events::{EventRouter, ProviderCandidate, Resolver};
use fiex_provider_contracts::{
    ProviderError, ProviderInvocation, ProviderRequest, ToolbarProvider, ToolbarResult,
};
use serde_json::{Value, json};
use std::sync::Arc;

struct FailedToolbarProvider(String);
impl ToolbarProvider for FailedToolbarProvider {
    fn invoke(&self, _ctx: &dyn ProviderInvocation) -> Result<ToolbarResult, ProviderError> {
        Err(ProviderError::internal(self.0.clone()))
    }
}

fn belongs_to_default_owner(provider: &Resolver, plugin_id: &str) -> bool {
    provider.plugin.as_deref().is_some_and(|plugin| {
        plugin.id == plugin_id && provider.id == format!("plugin:{plugin_id}")
    })
}

/// Invoked from the native toolbar; never directly dispatch an unregistered
/// event or trust caller-supplied resource identities or provider instance IDs.
pub fn invoke(plugin_id: &str, directory: &str, paths: &[String]) -> Result<Value, String> {
    if paths.len() > 256 {
        return Err("too many toolbar targets".into());
    }
    let targets = if paths.is_empty() {
        vec![directory.to_owned()]
    } else {
        paths.to_vec()
    };
    let resources = targets
        .iter()
        .map(|path| crate::plugins::resource::resource_context::resolve(path))
        .collect::<Result<Vec<_>, _>>()?;
    let targets = crate::plugins::events::matching::match_resources(&resources);
    let providers = crate::plugins::provider_registry::candidates(
        "toolbar.invoke",
        &targets,
        directory,
        Vec::new(),
    )
    .into_iter()
    .filter(|provider| belongs_to_default_owner(provider, plugin_id))
    .map(|resolver| {
        let implementation: Arc<dyn ToolbarProvider> = match resolver.plugin.as_deref() {
            Some(plugin) => match proxy::ScopedToolbarProxy::new(
                plugin.clone(),
                &resources,
                directory,
                resolver.id.clone(),
            ) {
                Ok(proxy) => Arc::new(proxy),
                Err(error) => Arc::new(FailedToolbarProvider(error)),
            },
            None => Arc::new(FailedToolbarProvider(
                "unregistered native ToolbarProvider".into(),
            )),
        };
        ProviderCandidate {
            resolver,
            implementation,
        }
    })
    .collect();
    let request = ProviderRequest {
        method: "toolbar.invoke".into(),
        params: json!({}),
    };
    let invocation = crate::plugins::native_invocation::NativeProviderInvocation::new(
        request.method.clone(),
        request.params.clone(),
        &resources,
        None,
    );
    let result = EventRouter::resolve_shared(
        "toolbar.invoke",
        providers,
        &invocation,
        |_, provider, context| provider.invoke(context).map_err(|error| error.to_string()),
        |_, result| {
            let wire = serde_json::to_value(result).map_err(|error| error.to_string())?;
            crate::plugins::events::validate_result("toolbar.invoke", &wire)?;
            Ok(result)
        },
        |_| true,
        |results| {
            results
                .into_iter()
                .next()
                .ok_or_else(|| {
                    "toolbar provider is disabled or does not match targets".to_string()
                })?
                .1
        },
    );
    serde_json::to_value(result).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_provider_cannot_be_used_as_plugin_toolbar_owner() {
        assert!(!belongs_to_default_owner(
            &Resolver::builtin("core.toolbar", 100),
            "foo"
        ));
    }

    #[test]
    fn empty_selection_uses_directory_and_rejects_bad_resource() {
        assert!(invoke("org.example.toolbar", "not-found://mount", &[]).is_err());
    }
}
