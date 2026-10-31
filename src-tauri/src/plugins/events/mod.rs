//! Core event pipeline: discover → dispatch → validate → collect.
use crate::plugins::resource::resource_context::ResourceSnapshot;
use serde_json::{json, Value};
pub(crate) mod matching;
pub use matching::metadata as resource_metadata;
mod directory_scope;
mod owner;
mod registry;
mod router;
mod scope;
mod validation;
use fiex_provider_contracts::{event, Dispatch};
pub use registry::{Handler, Resolver};
pub use router::EventRouter;
#[cfg(test)]
use scope::action_folders;
pub use scope::{prepare, resource_scope};
pub use validation::validate_result;
/// Core-owned system events are not exposed through the plugin UI bridge.
/// Unknown methods remain legal plugin-local business methods.
pub fn is_protected_method(method: &str) -> bool {
    event(method).is_some()
        || [
            "host.",
            "fs.",
            "core.",
            "plugin.",
            "contextMenu.",
            "connectionContextMenu.",
            "vfs.",
            "connection.",
        ]
        .iter()
        .any(|prefix| method.starts_with(prefix))
}

fn require_event(method: &str) -> Result<(), String> {
    if event(method).is_some() {
        Ok(())
    } else {
        Err(format!("unregistered Core event: {method}"))
    }
}

/// Recheck a stored menu session using its actual targets, never empty placeholders.
pub fn menu_allowed(
    plugin_id: &str,
    provider_id: &str,
    resources: &[ResourceSnapshot],
    directory: &str,
) -> bool {
    crate::plugins::provider_registry::candidates(
        "contextMenu.resolve",
        &matching::metadata(resources),
        directory,
        Vec::new(),
    )
    .iter()
    .any(|provider| {
        provider.id == provider_id
            && matches!(&provider.handler, Handler::Plugin(plugin) if plugin.id == plugin_id)
    })
}

pub fn call_owner(
    method: &str,
    plugin_id: &str,
    params: Value,
    resources: &[ResourceSnapshot],
    directory: Option<&str>,
) -> Result<Value, String> {
    let metadata = matching::metadata(resources);
    let providers = crate::plugins::provider_registry::candidates(
        method,
        &metadata,
        directory.unwrap_or(""),
        Vec::new(),
    )
    .into_iter()
    .filter(|p| {
        let Handler::Plugin(plugin) = &p.handler else {
            return false;
        };
        if plugin.id != plugin_id {
            return false;
        }
        match params.get("providerId") {
            Some(value) => value
                .as_str()
                .is_some_and(|instance| p.id == format!("plugin:{plugin_id}:{instance}")),
            None => p.id == format!("plugin:{plugin_id}"),
        }
    })
    .collect();
    let request = fiex_provider_contracts::ProviderRequest {
        method: method.into(),
        params,
    };
    EventRouter::resolve(
        method,
        providers,
        &request,
        |provider| match &provider.handler {
            Handler::Plugin(plugin) => Ok(Box::new(owner::PluginOwner {
                plugin: (**plugin).clone(),
                resources,
                directory,
            })
                as Box<dyn fiex_provider_contracts::ProviderEndpoint + '_>),
            Handler::Builtin(_) => Err("native owner is not registered".into()),
        },
        |_, value| Ok(value),
        |_| true,
        |results| {
            results
                .into_iter()
                .next()
                .ok_or_else(|| "event owner unavailable or does not match targets".to_string())?
                .1
        },
    )
}

pub fn invoke(plugin: &str, method: &str, params: Value) -> Result<Value, String> {
    require_event(method)?;
    let result = crate::plugins::host::global().invoke(plugin, method, params)?;
    validate_result(method, &result)?;
    Ok(result)
}
pub fn invoke_prepared(
    plugin: &str,
    method: &str,
    params: Value,
    scope: crate::plugins::resource::fs_access::Scope,
) -> Result<Value, String> {
    require_event(method)?;
    let value =
        crate::plugins::host::global().invoke_scoped(plugin, method, params, Some(scope))?;
    validate_result(method, &value)?;
    Ok(value)
}

pub fn invoke_resources(
    plugin: &crate::plugins::host::PluginInfo,
    method: &str,
    params: Value,
    resources: &[ResourceSnapshot],
    directory: Option<&str>,
) -> Result<Value, String> {
    let (params, scope) = resource_scope(plugin, method, params, resources, directory)?;
    let result =
        crate::plugins::host::global().invoke_scoped(&plugin.id, method, params, Some(scope))?;
    validate_result(method, &result)?;
    Ok(result)
}

#[cfg(test)]
mod tests;
