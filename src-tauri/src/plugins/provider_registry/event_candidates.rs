//! Typed event candidate discovery lives beside the VFS capability registry.
//! Matching and permission checks run before creating any IPC proxy.
use crate::plugins::events::{matching, Handler, Resolver};
use fiex_provider_contracts::{event, ProviderDescriptor, ProviderKind, ProviderMatchRule};
use serde_json::Value;

/// Candidate identity and priority obey the same contract as native entries.
fn valid_contribution(id: &str, kind: ProviderKind, priority: i64) -> bool {
    ProviderDescriptor {
        id: id.into(),
        kind,
        priority,
        r#match: ProviderMatchRule::default(),
    }
    .validate()
    .is_ok()
}

pub fn candidates(
    method: &str,
    metadata: &[Value],
    directory: &str,
    mut native: Vec<Resolver>,
) -> Vec<Resolver> {
    let Some(event) = event(method) else {
        return native;
    };
    let Some(kind) = event.kind() else {
        return native;
    };
    // Prune unrelated packages before computing runtime permissions, resolving
    // backend executables, or checking command availability. Runtime grants
    // are still checked afresh after discovery; the manifest is not trusted.
    for plugin in crate::plugins::host::catalog_matching(|manifest| {
        manifest
            .extensions
            .get(event.extension)
            .is_some_and(|value| value.is_object() || value == true)
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
        let instances = contribution["providers"].as_array();
        let count = instances.map_or(1, Vec::len);
        for index in 0..count {
            let mut plugin = plugin.clone();
            let mut contribution = contribution.clone();
            let mut instance_name = None;
            if let Some(instances) = instances {
                let instance = &instances[index];
                let Some(id) = instance["id"].as_str() else {
                    continue;
                };
                instance_name = Some(id.to_owned());
                let Some(fields) = contribution.as_object_mut() else {
                    continue;
                };
                fields.remove("providers");
                if let Some(extra) = instance.as_object() {
                    for (key, value) in extra {
                        if key != "id" {
                            fields.insert(key.clone(), value.clone());
                        }
                    }
                }
            }
            plugin.extensions[event.extension] = contribution.clone();
            if !matching::matches(method, &plugin.extensions, metadata, directory) {
                continue;
            }
            let priority = contribution
                .get("priority")
                .and_then(Value::as_i64)
                .unwrap_or(50);
            let instance_id = instance_name.as_deref().unwrap_or(&plugin.id);
            if !valid_contribution(instance_id, kind, priority) {
                continue;
            }
            native.push(Resolver {
                id: match instance_name {
                    Some(id) => format!("plugin:{}:{id}", plugin.id),
                    None => format!("plugin:{}", plugin.id),
                },
                name: plugin.name.clone(),
                priority,
                handler: Handler::Plugin(Box::new(plugin)),
            });
        }
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

    #[test]
    fn malicious_or_malformed_ipc_provider_ids_and_priorities_are_rejected() {
        assert!(valid_contribution("menu.edit", ProviderKind::ContextMenu, 100));
        assert!(!valid_contribution("invalid/id", ProviderKind::ContextMenu, 100));
        assert!(!valid_contribution("", ProviderKind::Info, 0));
        assert!(!valid_contribution("feature", ProviderKind::Info, i64::MAX));
    }
}
