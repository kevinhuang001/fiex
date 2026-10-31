use super::dynamic::{decode, merge};
use super::types::{Decoration, DecorationEntry};
use fiex_provider_contracts::{ProviderEndpoint, ProviderRequest};
use crate::plugins::host as plugin_host;
use serde_json::{json, Value};
use std::collections::HashMap;

fn bind_targets(
    response: &mut Value,
    resources: &[crate::plugins::resource::resource_context::ResourceSnapshot],
) {
    if let Some(items) = response["decorations"].as_array_mut() {
        for item in items {
            let target = item.get("ref").and_then(|reference| {
                resources.iter().find(|resource| {
                    serde_json::to_value(&resource.reference).ok().as_ref() == Some(reference)
                })
            });
            if let Some(target) = target {
                let path = target.display.clone();
                if let Some(object) = item.as_object_mut() {
                    object.remove("name");
                    object.remove("path");
                }
                item["path"] = json!(path);
            } else {
                *item = json!({});
            }
        }
    }
}

pub fn query(directory: &str, entries: Vec<DecorationEntry>) -> Vec<Decoration> {
    query_with_plugins(directory, entries, true)
}

pub fn query_with_plugins(
    directory: &str,
    entries: Vec<DecorationEntry>,
    include_plugins: bool,
) -> Vec<Decoration> {
    let entries = entries
        .into_iter()
        .map(|entry| (entry.path.clone(), entry))
        .collect::<HashMap<_, _>>();
    use crate::plugins::events::{EventRouter, Handler};
    let mut ordered = entries.values().collect::<Vec<_>>();
    ordered.sort_by(|a, b| a.path.cmp(&b.path));
    let resources = ordered
        .iter()
        .map(|e| {
            crate::plugins::resource::resource_context::ResourceSnapshot::from_entry(
                &e.path,
                &crate::core::files::Entry {
                    path: e.path.clone(),
                    name: e.name.clone(),
                    is_dir: e.is_dir,
                    extension: e.extension.clone(),
                    mime: e.mime.clone(),
                    readonly: e.readonly,
                    allowed_ops: e.allowed_ops.clone(),
                    ..Default::default()
                },
            )
        })
        .collect::<Vec<_>>();
    let metadata = crate::plugins::events::resource_metadata(&resources);
    let builtins = super::native::providers();
    let providers = if include_plugins {
        crate::plugins::provider_registry::candidates(
            "decorations.resolve",
            &metadata,
            directory,
            builtins,
        )
    } else {
        builtins
    };
    let manifests = plugin_host::installed_manifests()
        .into_iter()
        .map(|(m, f)| (m.id.clone(), (m, f)))
        .collect::<HashMap<_, _>>();
    let request = ProviderRequest {
        method: "decorations.resolve".into(),
        params: json!({}),
    };
    let result = EventRouter::resolve(
        "decorations.resolve",
        providers,
        &request,
        |resolver| {
            let endpoint: Box<dyn ProviderEndpoint + '_> = match &resolver.handler {
                Handler::Builtin(id) => Box::new(super::endpoint::NativeDecorationEndpoint {
                    id: (*id).into(),
                    directory,
                    entries: &entries,
                }),
                Handler::Plugin(plugin) => Box::new(super::endpoint::PluginDecorationProxy {
                    plugin: (**plugin).clone(),
                    directory,
                    resources: &resources,
                }),
            };
            Ok(endpoint)
        },
        |provider, response| {
            if matches!(provider.handler, Handler::Builtin(_)) {
                return serde_json::from_value::<Vec<Decoration>>(response["decorations"].clone())
                    .map_err(|error| error.to_string());
            }
            let Handler::Plugin(plugin) = &provider.handler else {
                unreachable!()
            };
            let (manifest, folder) = manifests.get(&plugin.id).ok_or("provider removed")?;
            let mut response = response;
            bind_targets(&mut response, &resources);
            decode(&entries, manifest, folder, response)
        },
        |_| true,
        |outcomes| {
            let mut merged = HashMap::new();
            for (provider, result) in outcomes.into_iter().rev() {
                if let Ok(values) = result {
                    for value in values {
                        let path = value.path.clone();
                        merge(
                            &mut merged,
                            &entries,
                            &path,
                            provider.priority.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
                            &provider.name,
                            value,
                        );
                    }
                }
            }
            merged
        },
    );
    let mut values = result
        .into_values()
        .map(|pending| pending.value)
        .collect::<Vec<_>>();
    values.sort_by(|a, b| a.path.cmp(&b.path));
    values
}

#[cfg(test)]
mod ref_target_tests {
    use super::*;
    use crate::plugins::{
        contracts::ResourceMetadata, resource::resource_context::ResourceSnapshot,
    };

    #[test]
    fn plugin_names_and_paths_never_select_decoration_targets() {
        let target = ResourceSnapshot::from_metadata(
            "/authorized/matching-name",
            ResourceMetadata::default(),
        );
        let reference = serde_json::to_value(&target.reference).unwrap();
        let mut payload = json!({"decorations":[
            {"name":"matching-name","path":"/authorized/matching-name","badge":{"text":"X"}},
            {"ref":{"mountId":"forged","nodeId":"forged"},"badge":{"text":"X"}},
            {"ref":reference,"name":"different-name","path":"/forged/other","badge":{"text":"Y"}}
        ]});
        bind_targets(&mut payload, &[target]);
        assert_eq!(payload["decorations"][0], json!({}));
        assert_eq!(payload["decorations"][1], json!({}));
        assert_eq!(
            payload["decorations"][2]["path"],
            json!("/authorized/matching-name")
        );
        assert!(payload["decorations"][2].get("name").is_none());
    }
}
