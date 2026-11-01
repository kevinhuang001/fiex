//! Capability-gated builtin context-menu providers.
//!
//! The menu is a list of small resolvers. A group exists only when its
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
use super::connection_cache::ConnectionCache;
use super::builtin::Context;
use crate::core::files::Entry;
use crate::plugins::native_invocation::NativeProviderInvocation;
use crate::plugins::provider_registry::native::NativeProviderRegistry;
use crate::plugins::resource::file_capabilities::FileCapabilities;
use crate::plugins::resource::resource_context::ResourceSnapshot;
use fiex_provider_contracts::{
    ContextMenuProvider, LocationMatch, MatchResource, MenuExecuteResult, MenuItem,
    MenuPlacement, ProviderError,
    ProviderInput, ProviderInvocation, ProviderMatchRule,
};
#[cfg(test)]
use fiex_provider_contracts::ProviderKind;
use serde_json::{json, Value};
use std::sync::Arc;

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

/// Every generic Core menu group is registered as the same shared provider
/// trait used by SDK plugins. Native implementations run directly in Core.
pub(super) fn register(
    values: &mut NativeProviderRegistry<dyn ContextMenuProvider>,
) {
    let implementations: [(&str, i64, Arc<dyn ContextMenuProvider>); 10] = [
        (
            "builtin:opening",
            100,
            Arc::new(Opening) as Arc<dyn ContextMenuProvider>,
        ),
        (
            "builtin:tags",
            99,
            Arc::new(Tags) as Arc<dyn ContextMenuProvider>,
        ),
        (
            "builtin:create",
            98,
            Arc::new(Create) as Arc<dyn ContextMenuProvider>,
        ),
        (
            "builtin:connection-cache",
            98,
            Arc::new(ConnectionCache) as Arc<dyn ContextMenuProvider>,
        ),
        (
            "builtin:rename",
            96,
            Arc::new(Rename) as Arc<dyn ContextMenuProvider>,
        ),
        (
            "builtin:batch-rename",
            95,
            Arc::new(BatchRename) as Arc<dyn ContextMenuProvider>,
        ),
        (
            "builtin:clipboard",
            94,
            Arc::new(Clipboard) as Arc<dyn ContextMenuProvider>,
        ),
        (
            "builtin:terminal",
            93,
            Arc::new(Terminal) as Arc<dyn ContextMenuProvider>,
        ),
        (
            "builtin:copy-path",
            92,
            Arc::new(CopyPath) as Arc<dyn ContextMenuProvider>,
        ),
        (
            "builtin:trash",
            91,
            Arc::new(Trash) as Arc<dyn ContextMenuProvider>,
        ),
    ];
    for (id, priority, implementation) in implementations {
        let rule = match id {
            "builtin:tags" | "builtin:terminal" => ProviderMatchRule {
                storage: Some(LocationMatch {
                    provider_ids: Some(vec!["core:local-fs".into()]),
                    ..Default::default()
                }),
                ..Default::default()
            },
            "builtin:connection-cache" => ProviderMatchRule {
                storage: Some(LocationMatch {
                    types: Some(vec!["vfs".into()]),
                    ..Default::default()
                }),
                ..Default::default()
            },
            _ => ProviderMatchRule::default(),
        };
        values
            .register(id, priority, rule, implementation)
            .expect("invalid builtin menu registration");
    }
}

fn registry() -> &'static NativeProviderRegistry<dyn ContextMenuProvider> {
    super::native_registry()
}

pub(super) fn invocation(context: &Context, method: &str, params: Value) -> NativeProviderInvocation {
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
    let provider = registry().get(id).ok_or("unknown native ContextMenuProvider")?;
    let invocation = invocation(context, "contextMenu.resolve", json!({}));
    let descriptor = registry().descriptor(id).ok_or("missing native menu descriptor")?;
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

pub(super) fn invoke(
    id: &str,
    invocation: &dyn ProviderInvocation,
) -> Result<Value, String> {
    let provider = registry().get(id).ok_or("unknown native ContextMenuProvider")?;
    let descriptor = registry().descriptor(id).ok_or("missing native menu descriptor")?;
    let resources = NativeProviderInvocation::resources(invocation)
        .map_err(|error| error.to_string())?;
    let targets = crate::plugins::events::matching::match_resources(resources);
    let directory = NativeProviderInvocation::directory(invocation)
        .ok()
        .and_then(|resource| crate::plugins::match_rules::directory_match_resource(&resource.display));
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
            .and_then(|result| serde_json::to_value(result).map_err(|error| ProviderError::internal(error.to_string())))
            .map_err(|error| error.to_string()),
        method => Err(ProviderError::method_not_found(method).to_string()),
    }
}

#[cfg(test)]
pub(super) fn execute(id: &str, item_id: &str, context: &Context) -> Result<Value, String> {
    let invocation = invocation(
        context,
        "contextMenu.execute",
        json!({"itemId": item_id}),
    );
    invoke(id, &invocation)
}

macro_rules! impl_context_menu_provider {
    ($($provider:ty),+ $(,)?) => {
        $(
            impl ContextMenuProvider for $provider {
                fn resolve(
                    &self,
                    invocation: &dyn ProviderInvocation,
                ) -> Result<Vec<MenuItem>, ProviderError> {
                    if invocation.method() != "contextMenu.resolve" {
                        return Err(ProviderError::method_not_found(invocation.method()));
                    }
                    let context = NativeProviderInvocation::local_data::<Context>(invocation)?;
                    let selection = context.selection();
                    let capabilities = context.capabilities();
                    if self.available(&selection, &capabilities) {
                        Ok(super::builtin::retain_unclaimed(
                            context,
                            self.items(&selection, &capabilities),
                        ))
                    } else {
                        Ok(Vec::new())
                    }
                }

                fn execute(
                    &self,
                    invocation: &dyn ProviderInvocation,
                ) -> Result<MenuExecuteResult, ProviderError> {
                    if invocation.method() != "contextMenu.execute" {
                        return Err(ProviderError::method_not_found(invocation.method()));
                    }
                    let items = <Self as ContextMenuProvider>::resolve(
                        self,
                        &NativeProviderInvocation::context_menu_resolve(invocation)?
                            .with_local_data(Arc::new(
                                NativeProviderInvocation::local_data::<Context>(invocation)?.clone(),
                            )),
                    )?;
                    let item_id = match invocation.input()? {
                        ProviderInput::ContextMenuExecute(request) => request.item_id.as_deref(),
                        _ => None,
                    }.ok_or_else(|| ProviderError::invalid_params("menu execute requires an itemId"))?;
                    if !items.iter().any(|item| item.id == item_id && !item.disabled) {
                        return Err(ProviderError::invalid_params("native menu action was not offered"));
                    }
                    Ok(super::builtin::action_result(
                        item_id,
                        NativeProviderInvocation::local_data::<Context>(invocation)?,
                    ))
                }
            }
        )+
    };
}

struct Opening;
impl Opening {
    fn available(&self, selection: &Selection<'_>, _: &FileCapabilities) -> bool {
        !selection.paths.is_empty()
    }
    fn items(&self, selection: &Selection<'_>, capabilities: &FileCapabilities) -> Vec<MenuItem> {
        // Directories navigate without reading; a file needs `read` to open.
        let navigable = selection
            .entries
            .iter()
            .all(|entry| entry.is_dir || capabilities.read);
        let mut items = Vec::new();
        if navigable {
            items.push(item("open", "打开", "open", "open"));
        }
        if !selection.many() {
            if capabilities.read {
                items.push(item("preview", "快速查看", "eye", "quickLook"));
            }
            items.push(item("info", "显示简介", "info", "info"));
        }
        items
    }
}

struct Tags;
impl Tags {
    fn available(&self, selection: &Selection<'_>, _: &FileCapabilities) -> bool {
        selection.all_local && !selection.paths.is_empty()
    }
    fn items(&self, _selection: &Selection<'_>, _: &FileCapabilities) -> Vec<MenuItem> {
        if cfg!(target_os = "macos") {
            vec![item("tags", "标签…", "tags", "")]
        } else {
            Vec::new()
        }
    }
}

/// Blank-canvas actions: there is no selection, so only creating entries is
/// meaningful. Both items are capability-gated: `mkdir` for a folder and the
/// provider's `write` (parentId + name) for an empty file.
struct Create;
impl Create {
    fn available(&self, selection: &Selection<'_>, capabilities: &FileCapabilities) -> bool {
        selection.paths.is_empty() && (capabilities.create || capabilities.write)
    }
    fn items(&self, _selection: &Selection<'_>, capabilities: &FileCapabilities) -> Vec<MenuItem> {
        let mut items = Vec::new();
        if capabilities.create {
            items.push(item("newFolder", "新建文件夹…", "folder-plus", "newFolder"));
        }
        if capabilities.write {
            items.push(item("newFile", "新建空文件…", "file-plus", "newFile"));
        }
        items
    }
}

struct Rename;
impl Rename {
    fn available(&self, selection: &Selection<'_>, capabilities: &FileCapabilities) -> bool {
        capabilities.rename && !selection.read_only && !selection.paths.is_empty()
    }
    fn items(&self, selection: &Selection<'_>, _: &FileCapabilities) -> Vec<MenuItem> {
        if selection.many() {
            Vec::new()
        } else {
            vec![item("rename", "重新命名", "pencil", "rename")]
        }
    }
}

struct BatchRename;
impl BatchRename {
    fn available(&self, selection: &Selection<'_>, capabilities: &FileCapabilities) -> bool {
        capabilities.rename && !selection.read_only && !selection.paths.is_empty()
    }
    fn items(&self, selection: &Selection<'_>, _: &FileCapabilities) -> Vec<MenuItem> {
        if selection.many() {
            vec![item("batchRename", "批量重命名…", "pencil", "")]
        } else {
            Vec::new()
        }
    }
}

struct Clipboard;
impl Clipboard {
    fn available(&self, selection: &Selection<'_>, capabilities: &FileCapabilities) -> bool {
        !selection.paths.is_empty()
            && (capabilities.read
                || (capabilities.read && capabilities.remove && !selection.read_only))
    }
    fn items(&self, selection: &Selection<'_>, capabilities: &FileCapabilities) -> Vec<MenuItem> {
        let mut items = Vec::new();
        if capabilities.read {
            items.push(item("copy", "复制", "copy", "copy"));
        }
        if capabilities.read && capabilities.remove && !selection.read_only {
            items.push(item("cut", "剪切", "scissors", "cut"));
        }
        if capabilities.read && capabilities.write && !selection.read_only {
            items.push(item("duplicate", "创建副本", "copy", "duplicate"));
        }
        items
    }
}

struct Terminal;
impl Terminal {
    fn available(&self, selection: &Selection<'_>, _: &FileCapabilities) -> bool {
        selection.all_local && !selection.paths.is_empty()
    }
    fn items(&self, _: &Selection<'_>, _: &FileCapabilities) -> Vec<MenuItem> {
        vec![item("terminal", "Open in Terminal", "terminal", "")]
    }
}

struct CopyPath;
impl CopyPath {
    fn available(&self, selection: &Selection<'_>, _: &FileCapabilities) -> bool {
        !selection.paths.is_empty()
    }
    fn items(&self, _: &Selection<'_>, _: &FileCapabilities) -> Vec<MenuItem> {
        vec![item("copyPath", "复制路径", "copy", "")]
    }
}

struct Trash;
impl Trash {
    fn available(&self, selection: &Selection<'_>, capabilities: &FileCapabilities) -> bool {
        capabilities.remove && !selection.read_only && !selection.paths.is_empty()
    }
    fn items(&self, selection: &Selection<'_>, _: &FileCapabilities) -> Vec<MenuItem> {
        let mut items = if selection.all_local {
            // The platform Trash only accepts local paths. Windows exposes the
            // Recycle Bin under that name; the rest keep the Trash wording.
            let title = if cfg!(windows) {
                "移到回收站"
            } else {
                "移到废纸篓"
            };
            vec![item("trash", title, "trash", "trash")]
        } else {
            // Provider-backed mounts have no Trash. Their `vfs.remove` is the
            // permanent delete, so the menu says exactly that.
            vec![item("delete", "彻底删除", "trash", "")]
        };
        for item in &mut items {
            item.placement = Some(MenuPlacement::Tail);
            item.danger = true;
        }
        items
    }
}

impl_context_menu_provider!(
    Opening,
    Tags,
    Create,
    Rename,
    BatchRename,
    Clipboard,
    Terminal,
    CopyPath,
    Trash,
);

pub(super) fn item(id: &str, title: &'static str, icon: &str, shortcut: &str) -> MenuItem {
    // Availability decides presence; a builtin item is never greyed out.
    let mut item = MenuItem::action(id, crate::i18n::t(title));
    item.icon = Some(icon.into());
    item.shortcut = (!shortcut.is_empty()).then(|| shortcut.into());
    item
}

#[cfg(test)]
mod registry_tests {
    use super::*;

    #[test]
    fn generic_native_menus_are_distinct_registered_context_menu_providers() {
        let registered = registry();
        assert_eq!(registered.len(), 10);
        for descriptor in registered.descriptors() {
            assert_eq!(descriptor.kind, ProviderKind::ContextMenu);
            descriptor.validate().unwrap();
        }
        let sorted = registered.candidates(|_| true);
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

        let ctx = Context::for_test_caps(
            "/tmp",
            Vec::new(),
            Vec::new(),
            false,
            FileCapabilities::ALL,
        );
        assert_eq!(
            providers(&ctx)
                .into_iter()
                .map(|provider| provider.id)
                .collect::<Vec<_>>(),
            vec!["builtin:create".to_owned()]
        );
    }
}
