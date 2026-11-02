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
//! - `rename` / `batch-rename`: require `rename` in a writable directory.
//! - `clipboard`: `copy` requires `read`, `cut` requires `rename` in a writable
//!   directory, `duplicate` requires `read && write` in a writable directory.
//! - `trash`: requires `remove` in a writable directory. View-specific
//!   restore, purge and empty actions belong only to the registered view provider.
//! - `terminal` / `tags`: local selections only, because Core has no remote
//!   terminal or tag path.
//! - `copy-path`: always; it only uses the location Core already knows.
//! - `connection-cache`: remote selections only, unchanged otherwise.
//!
//! Folder creation is not a menu group: the toolbar owns the single
//! "new folder" affordance and disables it from the listing's `canCreate`
//! (provider `mkdir` ∩ the directory node's own restrictions).
//!
//! Presence decides availability: an unsupported operation is absent from the
//! menu, never emitted as a disabled item. [`crate::plugins::resource::file_capabilities`]
//! re-checks the same call on execute.
use super::builtin::Context;
use crate::core::files::Entry;
use crate::plugins::native_invocation::NativeProviderInvocation;
use crate::plugins::provider_registry::native::NativeProviderRegistry;
#[cfg(test)]
use crate::plugins::resource::file_capabilities::FileCapabilities;
use crate::plugins::resource::resource_context::ResourceSnapshot;
#[cfg(test)]
use fiex_provider_contracts::ProviderKind;
use fiex_provider_contracts::{ContextMenuProvider, MatchResource};
#[cfg(test)]
use fiex_provider_contracts::{ProviderError, ProviderInvocation};
use serde_json::{json, Value};
use std::sync::Arc;

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
    "builtin:create",
    "builtin:connection-cache",
    "builtin:rename",
    "builtin:batch-rename",
    "builtin:clipboard",
    "builtin:terminal",
    "builtin:copy-path",
    "builtin:trash",
];

/// The selection a menu is resolved for.
pub(super) struct Selection<'a> {
    pub(super) paths: &'a [String],
    pub(super) entries: &'a [Entry],
    pub(super) read_only: bool,
    pub(super) all_local: bool,
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
    let targets = context
        .entries
        .iter()
        .map(|entry| ResourceSnapshot::from_entry(&entry.path, entry))
        .collect::<Vec<_>>();
    let directory = context
        .directory_entry
        .as_ref()
        .map(|entry| ResourceSnapshot::from_entry(&context.directory, entry));
    NativeProviderInvocation::new(method, params, &targets, directory.as_ref())
        .with_local_data(Arc::new(context.clone()))
}

fn matcher_targets(context: &Context) -> Vec<MatchResource> {
    let targets = context
        .entries
        .iter()
        .map(|entry| ResourceSnapshot::from_entry(&entry.path, entry))
        .collect::<Vec<_>>();
    crate::plugins::events::matching::match_resources(&targets)
}

pub(super) fn providers(context: &Context) -> Vec<crate::plugins::events::Resolver> {
    let targets = matcher_targets(context);
    let directory = crate::plugins::match_rules::directory_match_resource(&context.directory);
    let invocation = invocation(context, "contextMenu.resolve", json!({}));
    registry()
        .matched_candidates(&targets, directory.as_ref(), context.paths.len())
        .into_iter()
        .filter(|candidate| PROVIDER_IDS.contains(&candidate.id.as_str()))
        .filter(|candidate| {
            registry().get(&candidate.id).is_some_and(|provider| {
                provider
                    .resolve(&invocation)
                    .is_ok_and(|items| !items.is_empty())
            })
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
    let targets = matcher_targets(context);
    let directory = crate::plugins::match_rules::directory_match_resource(&context.directory);
    if !fiex_provider_contracts::ProviderMatcher::matches(
        &descriptor.r#match,
        &targets,
        directory.as_ref(),
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
        let ctx = Context::for_test_caps(
            "/tmp",
            Vec::new(),
            Vec::new(),
            false,
            FileCapabilities::default(),
        );
        assert!(providers(&ctx).is_empty());

        let ctx =
            Context::for_test_caps("/tmp", Vec::new(), Vec::new(), false, FileCapabilities::ALL);
        assert_eq!(
            providers(&ctx)
                .into_iter()
                .map(|provider| provider.id)
                .collect::<Vec<_>>(),
            vec!["builtin:create".to_owned()]
        );
    }
}
