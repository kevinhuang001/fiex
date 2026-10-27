use super::dynamic::{decode, merge};
use super::types::{Decoration, DecorationEntry};
use crate::plugins::host as plugin_host;
use serde_json::json;
use std::collections::HashMap;

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
    let mut resources = ordered
        .iter()
        .map(|e| {
            crate::plugins::resource::resource_context::ResourceSnapshot::from_metadata(
                &e.path,
                crate::plugins::contracts::ResourceMetadata {
                    name: e.name.clone(),
                    kind: if e.is_dir { "directory" } else { "file" }.into(),
                    extension: Some(e.extension.clone()),
                    mime: Some(e.mime.clone()),
                    size: None,
                    modified: None,
                },
            )
        })
        .collect::<Vec<_>>();
    if include_plugins {
        if let Ok(root) =
            crate::plugins::resource::resource_context::ResourceSnapshot::resolve(directory)
        {
            resources.insert(0, root);
        }
    }
    let metadata = resources
        .iter()
        .map(|r| serde_json::to_value(r.metadata()).unwrap_or_default())
        .collect::<Vec<_>>();
    let builtins = super::builtin::providers();
    let providers = if include_plugins {
        EventRouter::providers("decorations.resolve", &metadata, directory, builtins)
    } else {
        builtins
    };
    let manifests = plugin_host::installed_manifests()
        .into_iter()
        .map(|(m, f)| (m.id.clone(), (m, f)))
        .collect::<HashMap<_, _>>();
    let result = EventRouter::resolve(
        "decorations.resolve",
        providers,
        |_, id| super::builtin::resolve(id, &entries),
        |plugin| {
            crate::plugins::events::invoke_resources(
                plugin,
                "decorations.resolve",
                json!({"directory":directory}),
                &resources,
                Some(directory),
            )
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
