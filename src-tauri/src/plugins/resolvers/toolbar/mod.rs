//! Toolbar commands share the canonical discovery → scoped invocation →
//! validation pipeline, with a persistent IPC proxy per plugin identity.
//! Only the current selection snapshot crosses the operation boundary.
mod proxy;

use crate::plugins::{
    events::{EventRouter, Handler, Resolver},
    resource::resource_context::ResourceSnapshot,
};
use fiex_provider_contracts::ProviderRequest;
use serde_json::{json, Value};

fn belongs_to_default_owner(provider: &Resolver, plugin_id: &str) -> bool {
    matches!(
        &provider.handler,
        Handler::Plugin(plugin)
            if plugin.id == plugin_id && provider.id == format!("plugin:{plugin_id}")
    )
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
        .map(|path| ResourceSnapshot::resolve(path))
        .collect::<Result<Vec<_>, _>>()?;
    let metadata = crate::plugins::events::resource_metadata(&resources);
    let providers = crate::plugins::provider_registry::candidates(
        "toolbar.invoke",
        &metadata,
        directory,
        Vec::new(),
    )
    .into_iter()
    .filter(|provider| belongs_to_default_owner(provider, plugin_id))
    .collect();
    let request = ProviderRequest {
        method: "toolbar.invoke".into(),
        params: json!({}),
    };
    EventRouter::resolve(
        "toolbar.invoke",
        providers,
        &request,
        |provider| match &provider.handler {
            Handler::Plugin(plugin) => proxy::endpoint(plugin, &resources, directory),
            Handler::Builtin(_) => Err("native ToolbarProvider not registered".into()),
        },
        |_, result| Ok(result),
        |_| true,
        |results| {
            results
                .into_iter()
                .next()
                .ok_or_else(|| "toolbar provider is disabled or does not match targets".into())?
                .1
        },
    )
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
