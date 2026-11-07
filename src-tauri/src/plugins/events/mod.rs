//! Core event pipeline: discover → dispatch → validate → collect.
use crate::plugins::resource::resource_context::{ResourceSnapshot, ResourceSnapshotProjection};
use serde_json::{json, Value};
pub(crate) mod matching;
pub use matching::metadata as resource_metadata;
mod directory_scope;
mod registry;
mod router;
mod scope;
mod validation;
use fiex_provider_contracts::{event, Dispatch};
pub use registry::Resolver;
pub use router::{EventRouter, ProviderCandidate};
#[cfg(test)]
use scope::action_folders;
pub use scope::{prepare, resource_scope};
pub use validation::validate_result;

pub(crate) fn bind_provider_input(
    plugin_id: &str,
    registered_id: &str,
    input: &fiex_provider_contracts::ProviderInput,
) -> Result<fiex_provider_contracts::ProviderInput, String> {
    registry::bound_plugin_input(plugin_id, registered_id, input)
}

pub(crate) fn bind_provider_context(
    plugin_id: &str,
    registered_id: &str,
    context: &fiex_provider_contracts::ProviderRequestContext,
) -> Result<fiex_provider_contracts::ProviderRequestContext, String> {
    let mut bound = context.clone();
    bound.input = bind_provider_input(
        plugin_id,
        registered_id,
        context.input().map_err(|error| error.to_string())?,
    )?;
    bound.input_error = None;
    Ok(bound)
}
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
        &matching::match_resources(resources),
        directory,
        Vec::new(),
    )
    .iter()
    .any(|provider| {
        provider.id == provider_id
            && provider
                .plugin
                .as_deref()
                .is_some_and(|plugin| plugin.id == plugin_id)
    })
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

/// Info discovery and background calculations receive metadata-only scopes.
/// A user-triggered on-demand calculation gets a fresh action scope so its
/// provider can request the declared content permissions through Core.
pub fn invoke_resources_with_context(
    plugin: &crate::plugins::host::PluginInfo,
    method: &str,
    params: Value,
    resources: &[ResourceSnapshot],
    directory: Option<&str>,
    user_initiated: bool,
) -> Result<Value, String> {
    let (params, mut scope) = resource_scope(plugin, method, params, resources, directory)?;
    if !user_initiated {
        scope.restrict_to_metadata();
    } else {
        scope.set_user_action_context(
            crate::core::fs::mapped_access::UserActionContext::from_core_event(),
        );
    }
    let result =
        crate::plugins::host::global().invoke_scoped(&plugin.id, method, params, Some(scope))?;
    validate_result(method, &result)?;
    Ok(result)
}

#[cfg(test)]
mod tests;
