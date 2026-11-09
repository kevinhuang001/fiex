//! Native view menus are registered directly as shared ContextMenuProviders.
//! Their view-specific rules live in descriptors, using the same matcher as
//! plugin contributions.
use crate::plugins::events::Resolver;
use crate::plugins::native_invocation::NativeProviderInvocation;
use crate::plugins::provider_registry::native::NativeProviderRegistry;
use fiex_provider_contracts::{ContextMenuProvider, LocationMatch, ProviderMatchRule};
#[cfg(test)]
use fiex_provider_contracts::{
    ProviderError, ProviderInvocation, ProviderMatcher, ResourceMetadata,
};
use fiex_runtime::ModuleDescriptor;
use fiex_storage::CoreOwnerKey;
#[cfg(test)]
use serde_json::Value;
#[cfg(test)]
use serde_json::json;
use std::sync::Arc;

mod implementations;
use implementations::Reveal;

pub(super) fn providers_for_context(context: &super::builtin::Context) -> Vec<Resolver> {
    let registry = registry();
    registry
        .matched_candidates(
            &context.match_targets,
            context.directory_target.as_ref(),
            context.paths.len(),
        )
        .into_iter()
        .filter(|candidate| {
            registry.module_id(&candidate.id).as_deref() != Some(fiex_actions::MODULE_ID)
        })
        .collect()
}

pub(super) fn register(values: &mut NativeProviderRegistry<dyn ContextMenuProvider>) {
    let trash_module =
        fiex_trash::TrashModule::new(crate::core::volumes::native_vfs::trash_services());
    for (id, module_id, owner, storage_owner, selection, priority, implementation) in [
        (
            "builtin:reveal",
            "core:recent",
            "core:recent",
            CoreOwnerKey::new("recent"),
            "single",
            80,
            Arc::new(Reveal) as Arc<dyn ContextMenuProvider>,
        ),
        (
            "builtin:remove-from-recent",
            "core:recent",
            "core:recent",
            CoreOwnerKey::new("recent"),
            "nonzero",
            79,
            fiex_recent::RecentModule::new(crate::core::volumes::native_vfs::recent_services())
                .context_menu(),
        ),
        (
            "builtin:restore",
            "core:trash",
            "core:trash",
            fiex_trash::owner_key(),
            "single",
            20,
            trash_module.restore_menu(),
        ),
        (
            "builtin:empty-trash",
            "core:trash",
            "core:trash",
            fiex_trash::owner_key(),
            "zero",
            8,
            trash_module.empty_menu(),
        ),
    ] {
        values
            .module(ModuleDescriptor::new(
                module_id,
                storage_owner,
                env!("CARGO_PKG_VERSION"),
            ))
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

fn registry() -> NativeProviderRegistry<dyn ContextMenuProvider> {
    super::native_registry()
}

#[cfg(test)]
fn is_view_provider(id: &str) -> bool {
    registry()
        .module_id(id)
        .is_some_and(|module_id| module_id != fiex_actions::MODULE_ID)
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
            crate::plugins::resource::resource_context::from_metadata(
                path,
                ResourceMetadata {
                    kind: "file".into(),
                    ..Default::default()
                },
            )
        })
        .collect::<Vec<_>>();
    let directory_resource = crate::plugins::resource::resource_context::from_metadata(
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
            crate::plugins::resource::resource_context::from_metadata(
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
        .filter(|candidate| is_view_provider(&candidate.id))
        .collect()
}

#[cfg(test)]
pub(super) fn providers(directory: &str, paths: &[String]) -> Vec<Resolver> {
    matched(directory, paths)
}

#[cfg(test)]
pub(super) fn owns(id: &str) -> bool {
    is_view_provider(id)
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
