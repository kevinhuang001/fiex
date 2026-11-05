//! Atomic Core context-menu providers and their shared trusted selection data.
use super::builtin::Context;
use crate::core::files::Entry;
use crate::plugins::native_invocation::NativeProviderInvocation;
use crate::plugins::provider_registry::native::NativeProviderRegistry;
use fiex_provider_contracts::ContextMenuProvider;
#[cfg(test)]
use fiex_provider_contracts::ProviderKind;
#[cfg(test)]
use fiex_provider_contracts::{ProviderError, ProviderInvocation};
#[cfg(test)]
use fiex_resource::FileCapabilities;
use rayon::prelude::*;
#[cfg(test)]
use serde_json::json;
use serde_json::Value;
use std::sync::{Arc, OnceLock};

mod providers;
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

/// The selection a menu is resolved for.
pub(super) struct Selection<'a> {
    pub(super) paths: &'a [String],
    pub(super) entries: &'a [Entry],
    pub(super) all_local: bool,
    pub(super) preflight: super::builtin::MenuPreflightAvailability,
}

impl Selection<'_> {
    fn many(&self) -> bool {
        self.paths.len() > 1
    }
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
        .filter(|candidate| registry.module_id(&candidate.id).as_deref() == Some("core:actions"))
        .collect()
}

/// Resolve dynamic eligibility only for menu providers that passed their
/// static match and basic capability filters. The menu waits for these results
/// so its first-level contents are stable when displayed.
pub(super) fn preflight_availability(
    context: &Context,
) -> super::builtin::MenuPreflightAvailability {
    let registry = registry();
    let matched = registry
        .matched_candidates(
            &context.match_targets,
            context.directory_target.as_ref(),
            context.paths.len(),
        )
        .into_iter()
        .filter(|candidate| registry.module_id(&candidate.id).as_deref() == Some("core:actions"))
        .map(|candidate| candidate.id)
        .collect::<std::collections::HashSet<_>>();
    let capabilities = context.capabilities();
    let mut result = super::builtin::MenuPreflightAvailability::default();

    if context.paths.is_empty() {
        let check_file = capabilities.write && matched.contains("builtin:new-file");
        let check_folder = capabilities.create && matched.contains("builtin:new-folder");
        if check_file || check_folder {
            if let Ok(parent) = crate::core::fs::trusted(&context.directory, "core:context-menu") {
                result.new_file = check_file && parent.review_create_available(false);
                result.new_folder = check_folder && parent.review_create_available(true);
            }
        }
        return result;
    }

    let check_rename = capabilities.rename
        && ((context.paths.len() == 1 && matched.contains("builtin:rename"))
            || (context.paths.len() > 1 && matched.contains("builtin:batch-rename")));
    let check_trash = capabilities.trash && matched.contains("builtin:move-to-trash");
    let check_remove = capabilities.remove && matched.contains("builtin:delete-permanently");
    if !(check_rename || check_trash || check_remove) {
        return result;
    }

    let availability = preflight_for_paths(&context.paths, check_rename, check_trash, check_remove);
    let all_checked = !availability.is_empty();
    if context.paths.len() == 1 {
        result.rename =
            check_rename && all_checked && availability.iter().all(|value| value.rename);
    } else {
        result.batch_rename =
            check_rename && all_checked && availability.iter().all(|value| value.rename);
    }
    result.trash = check_trash && all_checked && availability.iter().all(|value| value.trash);
    result.remove = check_remove && all_checked && availability.iter().all(|value| value.remove);
    result
}

#[derive(Default)]
struct ResourceAvailability {
    rename: bool,
    trash: bool,
    remove: bool,
}

/// Reuse one trusted resource and run all applicable checks on one bounded
/// worker lane per selected path.
fn preflight_for_paths(
    paths: &[String],
    rename: bool,
    trash: bool,
    remove: bool,
) -> Vec<ResourceAvailability> {
    static POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();
    let pool = POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(8)
            .thread_name(|index| format!("menu-preflight-{index}"))
            .build()
            .expect("menu preflight pool configuration is valid")
    });
    let unique = paths.iter().collect::<std::collections::HashSet<_>>();
    pool.install(|| {
        unique
            .par_iter()
            .map(|path| {
                let Ok(resource) = crate::core::fs::trusted(path, "core:context-menu") else {
                    return ResourceAvailability::default();
                };
                ResourceAvailability {
                    rename: rename && resource.review_rename_available(),
                    trash: trash && resource.review_trash_available(),
                    remove: remove && resource.review_remove_available(),
                }
            })
            .collect::<Vec<_>>()
    })
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
        assert_eq!(generic.len(), 23);
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
        assert!(sorted
            .windows(2)
            .all(|pair| pair[0].priority >= pair[1].priority));
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
