//! Typed event candidate discovery lives beside the VFS capability registry.
//! Matching and permission checks run before creating any IPC proxy.
use crate::plugin_sdk::provider_contracts::event;
use crate::plugins::events::{matching, Handler, Resolver};
use serde_json::Value;

pub fn candidates(
    method: &str,
    metadata: &[Value],
    directory: &str,
    mut native: Vec<Resolver>,
) -> Vec<Resolver> {
    let Some(event) = event(method) else {
        return native;
    };
    for plugin in crate::plugins::host::catalog_matching(|manifest| {
        matching::matches(method, &manifest.extensions, metadata, directory)
    }) {
        if !plugin.enabled || !plugin.approved || !plugin.backend_available {
            continue;
        }
        let Some(contribution) = plugin.extensions.get(event.extension) else {
            continue;
        };
        if !contribution.is_object() && contribution.as_bool() != Some(true) {
            continue;
        }
        let declared = plugin.declared_permissions.iter().cloned().collect();
        let policy = crate::plugins::security::policy::PermissionPolicy::new(&plugin.id, &declared);
        let required = match method {
            "decorations.resolve" => {
                Some(crate::plugins::security::policy::Capability::FoldersRead)
            }
            "hooks.invoke" => Some(crate::plugins::security::policy::Capability::Hooks),
            _ => None,
        };
        if required.is_some_and(|capability| !policy.allowed(capability)) {
            continue;
        }
        native.push(Resolver {
            id: format!("plugin:{}", plugin.id),
            name: plugin.name.clone(),
            priority: contribution
                .get("priority")
                .and_then(Value::as_i64)
                .unwrap_or(50),
            handler: Handler::Plugin(Box::new(plugin)),
        });
    }
    native.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.id.cmp(&b.id)));
    crate::plugins::preferences::apply(event.extension, native)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unregistered_methods_never_claim_candidate_ownership() {
        let builtins = vec![Resolver::builtin("core.metadata", 100)];
        let candidates = candidates("not.an.event", &[], "", builtins);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].id, "core.metadata");
    }
}
