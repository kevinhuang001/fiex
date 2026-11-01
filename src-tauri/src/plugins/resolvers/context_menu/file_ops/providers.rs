//! Registered Core file-operation menu providers.
use super::{Context, Selection};
use crate::plugins::{
    native_invocation::NativeProviderInvocation, provider_registry::native::NativeProviderRegistry,
    resource::file_capabilities::FileCapabilities,
};
use fiex_provider_contracts::{
    ContextMenuProvider, LocationMatch, MenuExecuteResult, MenuItem, MenuPlacement, ProviderError,
    ProviderInput, ProviderInvocation, ProviderMatchRule,
};
use std::sync::Arc;

use super::super::connection_cache::ConnectionCache;

/// Every generic Core menu group is registered as the same shared provider
/// trait used by SDK plugins. Native implementations run directly in Core.
pub(super) fn register(values: &mut NativeProviderRegistry<dyn ContextMenuProvider>) {
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
                        Ok(super::super::builtin::retain_unclaimed(
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
                    Ok(super::super::builtin::action_result(
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
