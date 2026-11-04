//! Native view menus are registered directly as shared ContextMenuProviders.
//! Their view-specific rules live in descriptors, using the same matcher as
//! plugin contributions.
use crate::plugins::native_invocation::NativeProviderInvocation;
use crate::plugins::provider_registry::native::NativeProviderRegistry;
#[cfg(test)]
use crate::plugins::{events::Resolver, resource::resource_context::ResourceSnapshot};
use fiex_provider_contracts::{ContextMenuProvider, LocationMatch, ProviderMatchRule};
#[cfg(test)]
use fiex_provider_contracts::{
    ProviderError, ProviderInvocation, ProviderMatcher, ResourceMetadata,
};
#[cfg(test)]
use serde_json::json;
#[cfg(test)]
use serde_json::Value;
use std::sync::Arc;

mod implementations;
use implementations::{EmptyTrash, RemoveFromRecent, Restore, Reveal};

#[cfg(test)]
const PROVIDER_IDS: [&str; 4] = [
    "builtin:reveal",
    "builtin:remove-from-recent",
    "builtin:restore",
    "builtin:empty-trash",
];

pub(super) fn register(values: &mut NativeProviderRegistry<dyn ContextMenuProvider>) {
    for (id, owner, selection, priority, implementation) in [
        (
            "builtin:reveal",
            "core:recent",
            "single",
            80,
            Arc::new(Reveal) as Arc<dyn ContextMenuProvider>,
        ),
        (
            "builtin:remove-from-recent",
            "core:recent",
            "nonzero",
            79,
            Arc::new(RemoveFromRecent) as Arc<dyn ContextMenuProvider>,
        ),
        (
            "builtin:restore",
            "core:trash",
            "single",
            20,
            Arc::new(Restore) as Arc<dyn ContextMenuProvider>,
        ),
        (
            "builtin:empty-trash",
            "core:trash",
            "zero",
            8,
            Arc::new(EmptyTrash) as Arc<dyn ContextMenuProvider>,
        ),
    ] {
        values
            .register(
                id,
                priority,
                ProviderMatchRule {
                    selection: Some(selection.into()),
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

#[cfg(test)]
fn registry() -> &'static NativeProviderRegistry<dyn ContextMenuProvider> {
    super::native_registry()
}

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
pub(super) fn providers(directory: &str, paths: &[String]) -> Vec<Resolver> {
    matched(directory, paths)
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
