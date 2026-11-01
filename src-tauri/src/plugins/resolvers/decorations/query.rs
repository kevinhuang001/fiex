use super::dynamic::{decode, merge};
use super::types::{Decoration, DecorationEntry};
use crate::plugins::host as plugin_host;
use fiex_provider_contracts::ProviderRequest;
use serde_json::json;
use std::{collections::HashMap, sync::Arc};

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
    let match_targets = crate::plugins::events::matching::match_resources(&resources);
    let builtins = super::native::providers(&match_targets, directory);
    let providers = if include_plugins {
        crate::plugins::provider_registry::candidates(
            "decorations.resolve",
            &match_targets,
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
    let entries = Arc::new(entries);
    let resources = Arc::new(resources);
    let request = ProviderRequest {
        method: "decorations.resolve".into(),
        params: json!({}),
    };
    let result = EventRouter::resolve_typed(
        "decorations.resolve",
        providers,
        &request,
        |resolver, request| match &resolver.handler {
            Handler::Builtin(id) => {
                let invocation = super::invocation::CoreDecorationInvocation::native(
                    directory.into(),
                    entries.clone(),
                    resources.clone(),
                );
                super::native::invoke(id, &invocation)
            }
            Handler::Plugin(plugin) => {
                let contributions = super::proxy::ScopedDecorationProxy::new(
                    (**plugin).clone(),
                    directory,
                    entries.clone(),
                    resources.clone(),
                )?
                .resolve(request)?;
                let (manifest, folder) = manifests.get(&plugin.id).ok_or("provider removed")?;
                decode(
                    entries.as_ref(),
                    resources.as_slice(),
                    manifest,
                    folder,
                    contributions,
                )
            }
        },
        |_, values| {
            if values.len() > entries.len().max(1) {
                return Err("DecorationProvider returned too many items".into());
            }
            if values.iter().any(|value| !entries.contains_key(&value.path)) {
                return Err("DecorationProvider target is outside the scoped request".into());
            }
            Ok(values)
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
                            entries.as_ref(),
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
    use fiex_provider_contracts::{DecorationBadge, DecorationContribution};

    #[test]
    fn plugin_contributions_are_bound_only_by_core_resource_reference() {
        let target = ResourceSnapshot::from_metadata(
            "/authorized/matching-name",
            ResourceMetadata::default(),
        );
        let mut entries = HashMap::new();
        entries.insert(
            target.display.clone(),
            DecorationEntry {
                path: target.display.clone(),
                ..Default::default()
            },
        );
        let contribution = DecorationContribution {
            target: target.reference.clone(),
            badges: vec![DecorationBadge {
                text: "Y".into(),
                color: "blue".into(),
                tooltip: None,
            }],
            ..DecorationContribution::new(target.reference.clone())
        };
        assert_eq!(
            contribution.target,
            target.reference,
            "only the Core-provided opaque reference identifies a target"
        );
        assert!(entries.contains_key(&target.display));
        assert!(serde_json::from_value::<DecorationContribution>(json!({
            "ref":{"mountId":"forged","nodeId":"forged"},
            "name":"matching-name",
            "badge":{"text":"X"}
        }))
        .is_err());
    }
}
