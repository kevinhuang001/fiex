//! Capability-gated builtin context-menu providers.
//!
//! The menu is composed from small native providers. A group exists only when its
//! capability exists, so an unsupported rename/delete group is absent from the
//! resolver list instead of appearing with an empty item array.
//! The capability matrix is:
//!
//! - `opening`: always; `open` is omitted for a file entry without `read`
//!   (a directory opens by navigation and needs no read), `preview` requires
//!   `read`, `info` only reads Core metadata.
//! - `rename` / `batch-rename`: require `rename` for the selected nodes.
//! - `clipboard`: `copy` requires `read`, `cut` requires `rename`, and
//!   `duplicate` requires `read && write`.
//! - `trash`: requires `remove`. View-specific
//!   restore, purge and empty actions belong only to the registered view provider.
//! - `tags`: local selections only.
//! - `copy-path`: always; it only uses the location Core already knows.
//! - `connection-cache`: remote selections only, unchanged otherwise.
//!
//! Blank-canvas creation is split into separate New Folder and New File
//! providers. Their descriptors match `mkdir` and `write` independently;
//! New File can therefore exclude a view without hiding New Folder or
//! disabling VFS writes used by reviewed transfers.
//!
//! Presence decides availability: an unsupported operation is absent from the
//! menu, never emitted as a disabled item. [`crate::plugins::resource::file_capabilities`]
//! re-checks the same call on execute.
use super::builtin::Context;
use crate::core::files::Entry;
use crate::plugins::native_invocation::NativeProviderInvocation;
use crate::plugins::provider_registry::native::NativeProviderRegistry;
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

const PROVIDER_IDS: [&str; 10] = [
    "builtin:opening",
    "builtin:tags",
    "builtin:new-folder",
    "builtin:new-file",
    "builtin:connection-cache",
    "builtin:rename",
    "builtin:batch-rename",
    "builtin:clipboard",
    "builtin:copy-path",
    "builtin:trash",
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
    let selection = context.selection();
    let capabilities = context.capabilities();
    registry()
        .matched_candidates(
            &context.match_targets,
            context.directory_target.as_ref(),
            context.paths.len(),
        )
        .into_iter()
        .filter(|candidate| PROVIDER_IDS.contains(&candidate.id.as_str()))
        .filter(|candidate| {
            offered_item_ids(&candidate.id, &selection, capabilities)
                .is_some_and(|items| items.iter().any(|item| context.has_unclaimed_action(item)))
        })
        .collect()
}

/// Resolve dynamic eligibility only for menu providers that passed their
/// static match and basic capability filters. The menu waits for these results
/// so its first-level contents are stable when displayed.
pub(super) fn preflight_availability(
    context: &Context,
) -> super::builtin::MenuPreflightAvailability {
    use crate::plugin_sdk::filesystem::RemoveMode;

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
    let selection = context.selection();
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
    if !context.paths.is_empty() && capabilities.remove && matched.contains("builtin:trash") {
        let mode = if selection.all_local {
            RemoveMode::Trash
        } else {
            RemoveMode::Permanent
        };
        result.remove = all_preflight_allowed(&context.paths, |path| {
            crate::core::fs::trusted(path, "core:context-menu")
                .is_ok_and(|resource| resource.review_remove_available(mode))
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

/// Return the action IDs a native group can offer without constructing a
/// provider invocation. This keeps candidate selection cheap while retaining
/// the old behavior of omitting groups that would resolve to no items.
fn offered_item_ids(
    id: &str,
    selection: &Selection<'_>,
    capabilities: FileCapabilities,
) -> Option<Vec<&'static str>> {
    let items = match id {
        "builtin:opening" => {
            let mut items = Vec::new();
            if selection.paths.is_empty() {
                return Some(items);
            }
            if selection
                .entries
                .iter()
                .all(|entry| entry.is_dir || capabilities.read)
            {
                items.push("open");
            }
            if !selection.many() {
                if capabilities.read {
                    items.push("preview");
                }
                items.push("info");
            }
            items
        }
        "builtin:tags" => {
            if cfg!(target_os = "macos") && selection.all_local && !selection.paths.is_empty() {
                vec!["tags"]
            } else {
                Vec::new()
            }
        }
        "builtin:new-folder" => {
            if selection.paths.is_empty() && capabilities.create && selection.preflight.new_folder {
                vec!["newFolder"]
            } else {
                Vec::new()
            }
        }
        "builtin:new-file" => {
            if selection.paths.is_empty() && capabilities.write && selection.preflight.new_file {
                vec!["newFile"]
            } else {
                Vec::new()
            }
        }
        "builtin:connection-cache" => {
            if super::connection_cache::has_items_for_context(selection, &capabilities) {
                vec!["remote-action"]
            } else {
                Vec::new()
            }
        }
        "builtin:rename" => {
            if capabilities.rename && selection.paths.len() == 1 && selection.preflight.rename {
                vec!["rename"]
            } else {
                Vec::new()
            }
        }
        "builtin:batch-rename" => {
            if capabilities.rename && selection.many() && selection.preflight.batch_rename {
                vec!["batchRename"]
            } else {
                Vec::new()
            }
        }
        "builtin:clipboard" => {
            if !selection.paths.is_empty() && capabilities.read {
                vec!["copy"]
            } else {
                Vec::new()
            }
        }
        "builtin:copy-path" => {
            if selection.paths.is_empty() {
                Vec::new()
            } else {
                vec!["copyPath"]
            }
        }
        "builtin:trash" => {
            if capabilities.remove && !selection.paths.is_empty() && selection.preflight.remove {
                vec![if selection.all_local {
                    "trash"
                } else {
                    "delete"
                }]
            } else {
                Vec::new()
            }
        }
        _ => return None,
    };
    Some(items)
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
        assert_eq!(generic.len(), 10);
        for descriptor in generic {
            assert_eq!(descriptor.kind, ProviderKind::ContextMenu);
            descriptor.validate().unwrap();
        }
        let sorted = registered
            .candidates(|_| true)
            .into_iter()
            .filter(|candidate| candidate.id.starts_with("builtin:"))
            .collect::<Vec<_>>();
        assert_eq!(sorted[0].id, "builtin:opening");
        assert_eq!(sorted.last().unwrap().id, "builtin:trash");
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
                "builtin:new-file".to_owned(),
                "builtin:new-folder".to_owned(),
            ]
        );
    }
}
