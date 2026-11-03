//! Atomic Core context-menu providers and their shared trusted selection data.
use super::builtin::Context;
use crate::core::files::Entry;
use crate::plugins::native_invocation::NativeProviderInvocation;
use crate::plugins::provider_registry::native::NativeProviderRegistry;
#[cfg(test)]
use crate::plugins::resource::file_capabilities::FileCapabilities;
use fiex_provider_contracts::ContextMenuProvider;
#[cfg(test)]
use fiex_provider_contracts::ProviderKind;
#[cfg(test)]
use fiex_provider_contracts::{ProviderError, ProviderInvocation};
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

const PROVIDER_IDS: [&str; 22] = [
    "builtin:open",
    "builtin:preview",
    "builtin:info",
    "builtin:tags",
    "builtin:new-folder",
    "builtin:new-file",
    "builtin:rename",
    "builtin:batch-rename",
    "builtin:copy",
    "builtin:cut",
    "builtin:duplicate",
    "builtin:copy-path",
    "builtin:pin",
    "builtin:unpin",
    "builtin:cache",
    "builtin:uncache",
    "builtin:conflict-resolution",
    "builtin:move-to-trash",
    "builtin:delete-permanently",
    "builtin:reveal",
    "builtin:restore",
    "builtin:empty-trash",
];

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

fn registry() -> &'static NativeProviderRegistry<dyn ContextMenuProvider> {
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

pub(super) fn providers(context: &Context) -> Vec<crate::plugins::events::Resolver> {
    registry()
        .matched_candidates(
            &context.match_targets,
            context.directory_target.as_ref(),
            context.paths.len(),
        )
        .into_iter()
        .filter(|candidate| PROVIDER_IDS.contains(&candidate.id.as_str()))
        .collect()
}

/// Resolve dynamic eligibility only for menu providers that passed their
/// static match and basic capability filters. The menu waits for these results
/// so its first-level contents are stable when displayed.
pub(super) fn preflight_availability(
    context: &Context,
) -> super::builtin::MenuPreflightAvailability {
    let matched = registry()
        .matched_candidates(
            &context.match_targets,
            context.directory_target.as_ref(),
            context.paths.len(),
        )
        .into_iter()
        .filter(|candidate| PROVIDER_IDS.contains(&candidate.id.as_str()))
        .map(|candidate| candidate.id)
        .collect::<std::collections::HashSet<_>>();
    let capabilities = context.capabilities();
    let mut result = super::builtin::MenuPreflightAvailability::default();

    if context.paths.is_empty() && capabilities.write && matched.contains("builtin:new-file") {
        result.new_file = crate::core::fs::trusted(&context.directory, "core:context-menu")
            .is_ok_and(|parent| parent.review_create_available(false));
    }
    if context.paths.is_empty() && capabilities.create && matched.contains("builtin:new-folder") {
        result.new_folder = crate::core::fs::trusted(&context.directory, "core:context-menu")
            .is_ok_and(|parent| parent.review_create_available(true));
    }
    if context.paths.len() == 1 && capabilities.rename && matched.contains("builtin:rename") {
        result.rename = all_preflight_allowed(&context.paths, |path| {
            crate::core::fs::trusted(path, "core:context-menu")
                .is_ok_and(|resource| resource.review_rename_available())
        });
    }
    if context.paths.len() > 1 && capabilities.rename && matched.contains("builtin:batch-rename") {
        result.batch_rename = all_preflight_allowed(&context.paths, |path| {
            crate::core::fs::trusted(path, "core:context-menu")
                .is_ok_and(|resource| resource.review_rename_available())
        });
    }
    if !context.paths.is_empty() && capabilities.trash && matched.contains("builtin:move-to-trash")
    {
        result.trash = all_preflight_allowed(&context.paths, |path| {
            crate::core::fs::trusted(path, "core:context-menu")
                .is_ok_and(|resource| resource.review_trash_available())
        });
    }
    if !context.paths.is_empty()
        && capabilities.remove
        && matched.contains("builtin:delete-permanently")
    {
        result.remove = all_preflight_allowed(&context.paths, |path| {
            crate::core::fs::trusted(path, "core:context-menu")
                .is_ok_and(|resource| resource.review_remove_available())
        });
    }
    result
}

/// Keep synchronous menu resolution stable while limiting concurrent provider
/// checks to eight. Duplicate selections are checked once and share the cache.
fn all_preflight_allowed(paths: &[String], check: impl Fn(&str) -> bool + Sync) -> bool {
    static POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();
    let pool = POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(8)
            .thread_name(|index| format!("menu-preflight-{index}"))
            .build()
            .expect("menu preflight pool configuration is valid")
    });
    let unique = paths.iter().collect::<std::collections::HashSet<_>>();
    pool.install(|| unique.par_iter().all(|path| check(path)))
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
            .filter(|descriptor| descriptor.id.starts_with("builtin:"))
            .collect::<Vec<_>>();
        assert_eq!(generic.len(), 22);
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
