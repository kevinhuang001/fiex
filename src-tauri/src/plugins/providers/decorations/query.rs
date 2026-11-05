use super::dynamic::{decode, merge, path_for_target};
use super::types::{Decoration, DecorationEntry};
use crate::plugins::host as plugin_host;
use fiex_provider_contracts::{
    DecorationContribution, DecorationProvider, ProviderError, ProviderInvocation,
};
use serde_json::json;
use std::{collections::HashMap, sync::Arc};

struct FailedDecorationProvider(String);

impl DecorationProvider for FailedDecorationProvider {
    fn resolve(
        &self,
        _ctx: &dyn ProviderInvocation,
    ) -> Result<Vec<DecorationContribution>, ProviderError> {
        Err(ProviderError::internal(self.0.clone()))
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
    use crate::plugins::events::{EventRouter, ProviderCandidate};
    let mut ordered = entries.values().collect::<Vec<_>>();
    ordered.sort_by(|a, b| a.path.cmp(&b.path));
    let resources = ordered
        .iter()
        .map(|e| {
            crate::plugins::resource::resource_context::from_entry(
                &e.path,
                &crate::core::files::Entry {
                    path: e.path.clone(),
                    name: e.name.clone(),
                    is_dir: e.is_dir,
                    extension: e.extension.clone(),
                    mime: e.mime.clone(),
                    allowed_ops: e.allowed_ops.clone(),
                    ..Default::default()
                },
            )
        })
        .collect::<Vec<_>>();
    let targets = crate::plugins::events::matching::match_resources(&resources);
    let builtins = super::native::providers(&targets, directory);
    let providers = if include_plugins {
        crate::plugins::provider_registry::candidates(
            "decorations.resolve",
            &targets,
            directory,
            builtins,
        )
    } else {
        builtins
    };
    let manifests = if providers.iter().any(|provider| provider.plugin.is_some()) {
        plugin_host::installed_manifests()
            .into_iter()
            .map(|(m, f)| (m.id.clone(), (m, f)))
            .collect::<HashMap<_, _>>()
    } else {
        HashMap::new()
    };
    let entries = Arc::new(entries);
    let resources = Arc::new(resources);
    let providers = providers
        .into_iter()
        .map(|resolver| {
            let implementation: Arc<dyn DecorationProvider> = match resolver.plugin.as_deref() {
                None => super::native::provider(&resolver.id).unwrap_or_else(|| {
                    Arc::new(FailedDecorationProvider(
                        "registered native DecorationProvider disappeared".into(),
                    ))
                }),
                Some(plugin) => match super::proxy::ScopedDecorationProxy::new(
                    plugin.clone(),
                    &resolver.id,
                    directory,
                    entries.clone(),
                    resources.clone(),
                ) {
                    Ok(proxy) => Arc::new(proxy),
                    Err(error) => Arc::new(FailedDecorationProvider(error)),
                },
            };
            ProviderCandidate {
                resolver,
                implementation,
            }
        })
        .collect();
    let invocation = super::invocation::CoreDecorationInvocation::unbound(
        directory.into(),
        entries.clone(),
        resources.clone(),
    );
    let resources_by_ref = super::dynamic::resource_path_index(resources.as_slice());
    let result = EventRouter::resolve_merge_parallel(
        "decorations.resolve",
        providers,
        &invocation,
        |resolver, provider, context| {
            if resolver.plugin.is_none() {
                let native = super::invocation::CoreDecorationInvocation::native(
                    &resolver.id,
                    directory.into(),
                    entries.clone(),
                    resources.clone(),
                )?;
                provider.resolve(&native).map_err(|error| error.to_string())
            } else {
                provider.resolve(context).map_err(|error| error.to_string())
            }
        },
        |_, items| {
            let response = json!({"decorations": items});
            crate::plugins::events::validate_result("decorations.resolve", &response)?;
            Ok(items)
        },
        |outcomes| {
            let mut merged = HashMap::new();
            for (provider, result) in outcomes.into_iter().rev() {
                let Ok(items) = result else {
                    continue;
                };
                let values = if let Some(plugin) = provider.plugin.as_deref() {
                    manifests
                        .get(&plugin.id)
                        .ok_or_else(|| "provider removed".to_owned())
                        .and_then(|(manifest, folder)| {
                            decode(entries.as_ref(), &resources_by_ref, manifest, folder, items)
                        })
                } else {
                    Ok(items
                        .into_iter()
                        .filter_map(|contribution| {
                            let path = path_for_target(
                                &contribution.target,
                                entries.as_ref(),
                                &resources_by_ref,
                            )?
                            .to_owned();
                            Some(Decoration {
                                path,
                                native_badges: contribution.native_badges,
                                label: contribution.label,
                                description: contribution.description,
                                fields: super::dynamic::fields_with_name_contribution(
                                    contribution.fields,
                                    contribution.badges,
                                    contribution.style,
                                ),
                                icon: contribution.icon,
                                icon_path: contribution.icon_path,
                                providers: Vec::new(),
                            })
                        })
                        .collect::<Vec<_>>())
                };
                let Ok(values) = values else {
                    continue;
                };
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
    use crate::plugins::contracts::ResourceMetadata;

    #[test]
    fn only_core_issued_resource_refs_select_decoration_targets() {
        let mut entries = HashMap::new();
        entries.insert(
            "/authorized/matching-name".into(),
            DecorationEntry {
                path: "/authorized/matching-name".into(),
                ..Default::default()
            },
        );
        let target = crate::plugins::resource::resource_context::from_metadata(
            "/authorized/matching-name",
            ResourceMetadata::default(),
        );
        let resources = [target.clone()];
        let resources_by_ref = super::super::dynamic::resource_path_index(&resources);
        assert_eq!(
            path_for_target(&target.reference, &entries, &resources_by_ref),
            Some("/authorized/matching-name")
        );
        let forged = crate::plugins::resource::resource_context::from_metadata(
            "/forged/path",
            ResourceMetadata::default(),
        );
        assert_eq!(
            path_for_target(&forged.reference, &entries, &resources_by_ref),
            None
        );
    }
}
