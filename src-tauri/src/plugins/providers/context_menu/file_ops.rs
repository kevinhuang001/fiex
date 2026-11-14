//! Atomic Core context-menu providers and their shared trusted selection data.
use super::builtin::Context;
use crate::plugins::native_invocation::NativeProviderInvocation;
use crate::plugins::provider_registry::native::NativeProviderRegistry;
use fiex_provider_contracts::ContextMenuProvider;
#[cfg(test)]
use fiex_provider_contracts::ProviderKind;
#[cfg(test)]
use fiex_provider_contracts::{ProviderError, ProviderInvocation};
#[cfg(test)]
use fiex_resource::FileCapabilities;
use serde_json::Value;
#[cfg(test)]
use serde_json::json;
use std::sync::Arc;

mod preflight;
mod providers;
mod selection;
pub(super) use preflight::preflight_availability;
pub(super) use selection::Selection;
pub(super) fn register(values: &mut NativeProviderRegistry<dyn ContextMenuProvider>) {
    providers::register(values);
}

pub(super) fn item(
    id: &str,
    title: &'static str,
    icon: &str,
    shortcut: &str,
) -> fiex_provider_contracts::MenuItem {
    providers::item(id, title, icon, shortcut)
}

fn registry() -> NativeProviderRegistry<dyn ContextMenuProvider> {
    super::native_registry()
}

pub(super) fn invocation(
    context: &Context,
    method: &str,
    params: Value,
) -> NativeProviderInvocation {
    NativeProviderInvocation::new(
        method,
        params,
        context.resources.as_slice(),
        context.directory_resource.as_ref(),
    )
    .with_local_data(Arc::new(context.clone()))
}

pub(super) fn invocation_for(
    provider_id: &str,
    context: &Context,
    method: &str,
    params: Value,
) -> Result<NativeProviderInvocation, String> {
    registry()
        .invocation(
            provider_id,
            method,
            params,
            context.resources.as_slice(),
            context.directory_resource.as_ref(),
        )
        .map(|invocation| invocation.with_local_data(Arc::new(context.clone())))
}

pub(super) fn providers(context: &Context) -> Vec<crate::plugins::events::Resolver> {
    let registry = registry();
    registry
        .matched_candidates(
            &context.match_targets,
            context.directory_target.as_ref(),
            context.paths.len(),
        )
        .into_iter()
        .filter(|candidate| {
            registry.module_id(&candidate.id).as_deref() == Some(fiex_actions::MODULE_ID)
        })
        .collect()
}

#[cfg(test)]
pub(super) fn resolve(id: &str, context: &Context) -> Result<Value, String> {
    let provider = registry()
        .get(id)
        .ok_or("unknown native ContextMenuProvider")?;
    let invocation = invocation(context, "contextMenu.resolve", json!({}));
    let descriptor = registry()
        .descriptor(id)
        .ok_or("missing native menu descriptor")?;
    if !fiex_provider_contracts::ProviderMatcher::matches(
        &descriptor.r#match,
        &context.match_targets,
        context.directory_target.as_ref(),
        context.paths.len(),
    ) {
        return Err("native ContextMenuProvider does not match selection".into());
    }
    let items = provider
        .resolve(&invocation)
        .map_err(|error| error.to_string())?;
    Ok(json!({"items": items}))
}

#[cfg(test)]
pub(super) fn invoke(id: &str, invocation: &dyn ProviderInvocation) -> Result<Value, String> {
    let provider = registry()
        .get(id)
        .ok_or("unknown native ContextMenuProvider")?;
    let descriptor = registry()
        .descriptor(id)
        .ok_or("missing native menu descriptor")?;
    let resources =
        NativeProviderInvocation::resources(invocation).map_err(|error| error.to_string())?;
    let targets = crate::plugins::events::matching::match_resources(resources);
    let directory = NativeProviderInvocation::directory(invocation)
        .ok()
        .and_then(|resource| {
            crate::plugins::match_rules::directory_match_resource(&resource.display)
        });
    if !fiex_provider_contracts::ProviderMatcher::matches(
        &descriptor.r#match,
        &targets,
        directory.as_ref(),
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
pub(super) fn execute(id: &str, item_id: &str, context: &Context) -> Result<Value, String> {
    let invocation = invocation(context, "contextMenu.execute", json!({"itemId": item_id}));
    invoke(id, &invocation)
}

#[cfg(test)]
mod registry_tests {
    use super::*;

    #[test]
    fn generic_native_menus_are_distinct_registered_context_menu_providers() {
        let registered = registry();
        let generic = registered
            .descriptors()
            .into_iter()
            .filter(|descriptor| descriptor.id.starts_with("builtin:"))
            .collect::<Vec<_>>();
        assert_eq!(generic.len(), 24);
        for descriptor in generic {
            assert_eq!(descriptor.kind, ProviderKind::ContextMenu);
            descriptor.validate().unwrap();
        }
        let sorted = registered
            .candidates(|_| true)
            .into_iter()
            .filter(|candidate| candidate.id.starts_with("builtin:"))
            .collect::<Vec<_>>();
        assert_eq!(sorted[0].id, "builtin:open");
        assert_eq!(sorted.last().unwrap().id, "builtin:empty-trash");
        assert!(
            sorted
                .windows(2)
                .all(|pair| pair[0].priority >= pair[1].priority)
        );
        assert!(registered.get("builtin:nonexistent").is_none());
    }

    #[test]
    fn capabilities_are_checked_by_each_registered_native_menu() {
        let ctx =
            Context::for_test_caps("/tmp", Vec::new(), Vec::new(), FileCapabilities::default());
        assert!(providers(&ctx).is_empty());

        let ctx = Context::for_test_caps("/tmp", Vec::new(), Vec::new(), FileCapabilities::ALL);
        assert_eq!(
            providers(&ctx)
                .into_iter()
                .map(|provider| provider.id)
                .collect::<Vec<_>>(),
            vec![
                "builtin:new-folder".to_owned(),
                "builtin:new-file".to_owned(),
            ]
        );
    }
}
