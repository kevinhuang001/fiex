//! Native view menus are registered directly as shared ContextMenuProviders.
//! Their view-specific rules live in descriptors, using the same matcher as
//! plugin contributions.
use crate::plugins::{
    events::Resolver, native_invocation::NativeProviderInvocation,
    provider_registry::native::NativeProviderRegistry,
    resource::resource_context::ResourceSnapshot,
};
use fiex_provider_contracts::{
    ContextMenuProvider, LocationMatch, ProviderMatchRule, ResourceMetadata,
};
#[cfg(test)]
use fiex_provider_contracts::{MatchResource, ProviderError, ProviderInvocation, ProviderMatcher};
use serde_json::{json, Value};
use std::sync::Arc;

mod implementations;
use implementations::{ApplicationsMenu, RecentMenu, TrashMenu};

const PROVIDER_IDS: [&str; 3] = [
    "core.recent.menu",
    "core.applications.menu",
    "core.trash.menu",
];

pub(super) fn register(values: &mut NativeProviderRegistry<dyn ContextMenuProvider>) {
    for (id, owner, implementation) in [
        (
            "core.recent.menu",
            "core:recent",
            Arc::new(RecentMenu) as Arc<dyn ContextMenuProvider>,
        ),
        (
            "core.applications.menu",
            "core:applications",
            Arc::new(ApplicationsMenu) as Arc<dyn ContextMenuProvider>,
        ),
        (
            "core.trash.menu",
            "core:trash",
            Arc::new(TrashMenu) as Arc<dyn ContextMenuProvider>,
        ),
    ] {
        values
            .register(
                id,
                100,
                ProviderMatchRule {
                    view: Some(LocationMatch {
                        provider_ids: Some(vec![owner.into()]),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                implementation,
            )
            .expect("invalid native ContextMenuProvider registration");
    }
}

fn registry() -> &'static NativeProviderRegistry<dyn ContextMenuProvider> {
    super::native_registry()
}

fn context(
    directory: &str,
    paths: &[String],
    method: &str,
    params: Value,
) -> NativeProviderInvocation {
    let targets = paths
        .iter()
        .map(|path| {
            ResourceSnapshot::from_metadata(
                path,
                ResourceMetadata {
                    kind: "file".into(),
                    ..Default::default()
                },
            )
        })
        .collect::<Vec<_>>();
    let directory_resource = ResourceSnapshot::from_metadata(
        directory,
        ResourceMetadata {
            kind: "directory".into(),
            ..Default::default()
        },
    );
    NativeProviderInvocation::new(method, params, &targets, Some(&directory_resource))
}

fn matched(directory: &str, paths: &[String]) -> Vec<Resolver> {
    let snapshots = paths
        .iter()
        .map(|path| {
            ResourceSnapshot::from_metadata(
                path,
                ResourceMetadata {
                    kind: "file".into(),
                    ..Default::default()
                },
            )
        })
        .collect::<Vec<_>>();
    let targets = crate::plugins::events::matching::match_resources(&snapshots);
    let directory_context = crate::plugins::match_rules::directory_match_resource(directory);
    registry()
        .matched_candidates(&targets, directory_context.as_ref(), paths.len())
        .into_iter()
        .filter(|candidate| PROVIDER_IDS.contains(&candidate.id.as_str()))
        .collect()
}

pub(super) fn resolve_snapshot(directory: &str, paths: &[String]) -> (Vec<Resolver>, Vec<String>) {
    let invocation = context(directory, paths, "contextMenu.resolve", json!({}));
    let mut providers = Vec::new();
    let mut claimed = Vec::new();
    for candidate in matched(directory, paths) {
        let Some(provider) = registry().get(&candidate.id) else {
            continue;
        };
        let Ok(items) = provider.resolve(&invocation) else {
            continue;
        };
        if items.is_empty() {
            continue;
        }
        claimed.extend(items.into_iter().map(|item| item.id));
        providers.push(candidate);
    }
    (providers, claimed)
}

#[cfg(test)]
pub(super) fn providers(directory: &str, paths: &[String]) -> Vec<Resolver> {
    resolve_snapshot(directory, paths).0
}

#[cfg(test)]
pub(super) fn owns(id: &str) -> bool {
    PROVIDER_IDS.contains(&id)
}

#[cfg(test)]
pub(super) fn resolve(id: &str, directory: &str, paths: &[String]) -> Result<Value, String> {
    let invocation = context(directory, paths, "contextMenu.resolve", json!({}));
    invoke(id, &invocation)
}

#[cfg(test)]
pub(super) fn invoke(id: &str, invocation: &dyn ProviderInvocation) -> Result<Value, String> {
    let provider = registry()
        .get(id)
        .ok_or("unknown native ContextMenuProvider")?;
    let resources =
        NativeProviderInvocation::resources(invocation).map_err(|error| error.to_string())?;
    let targets = crate::plugins::events::matching::match_resources(resources);
    let directory_context = NativeProviderInvocation::directory(invocation)
        .ok()
        .and_then(|directory| {
            crate::plugins::match_rules::directory_match_resource(&directory.display)
        });
    let descriptor = registry()
        .descriptor(id)
        .ok_or("missing native menu descriptor")?;
    if !ProviderMatcher::matches(
        &descriptor.r#match,
        &targets,
        directory_context.as_ref(),
        invocation.request().selection_count,
    ) {
        return Err("native ContextMenuProvider does not match selection".into());
    }
    match invocation.method() {
        "contextMenu.resolve" => provider
            .resolve(invocation)
            .map(|items| json!({"items": items}))
            .map_err(|error| error.to_string()),
        "contextMenu.execute" => provider
            .execute(invocation)
            .and_then(|result| {
                serde_json::to_value(result)
                    .map_err(|error| ProviderError::internal(error.to_string()))
            })
            .map_err(|error| error.to_string()),
        method => Err(ProviderError::method_not_found(method).to_string()),
    }
}

#[cfg(test)]
pub(super) fn execute(
    id: &str,
    item_id: &str,
    directory: &str,
    paths: &[String],
) -> Result<Value, String> {
    let offered = resolve(id, directory, paths)?;
    if !offered["items"].as_array().is_some_and(|items| {
        items
            .iter()
            .any(|item| item["id"] == item_id && item["disabled"] != true)
    }) {
        return Err("native menu action was not offered".into());
    }
    let invocation = context(
        directory,
        paths,
        "contextMenu.execute",
        json!({"itemId": item_id}),
    );
    invoke(id, &invocation)
}

/// Construct the same shared invocation for a real menu request after Core has
/// resolved and authorized its selected entries.
#[cfg(test)]
mod tests;
