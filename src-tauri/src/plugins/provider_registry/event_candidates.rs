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
    for plugin in crate::plugins::host::catalog() {
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
            native.push(Resolver {
                id: match instance_name {
                    Some(id) => format!("plugin:{}:{id}", plugin.id),
                    None => format!("plugin:{}", plugin.id),
                },
                name: plugin.name.clone(),
                priority: contribution
                    .get("priority")
                    .and_then(Value::as_i64)
                    .unwrap_or(50),
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
}
