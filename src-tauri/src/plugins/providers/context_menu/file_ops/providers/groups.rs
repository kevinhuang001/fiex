use super::super::{Context, Selection};
use super::item;
use crate::plugins::{
    native_invocation::NativeProviderInvocation, resource::file_capabilities::FileCapabilities,
};
use fiex_provider_contracts::{
    ContextMenuProvider, MenuExecuteResult, MenuItem, MenuPlacement, ProviderError, ProviderInput,
    ProviderInvocation,
};
use std::sync::Arc;

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
                        Ok(super::super::super::builtin::retain_unclaimed(
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
                    Ok(super::super::super::builtin::action_result(
                        item_id,
                        NativeProviderInvocation::local_data::<Context>(invocation)?,
                    ))
                }
            }
        )+
    };
}

pub(super) struct Opening;
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

pub(super) struct Tags;
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

/// Blank-canvas directory creation. It has its own match rule so Applications
/// can keep New Folder while excluding New File.
pub(super) struct NewFolder;
impl NewFolder {
    fn available(&self, selection: &Selection<'_>, capabilities: &FileCapabilities) -> bool {
        selection.paths.is_empty() && capabilities.create
    }
    fn items(&self, _selection: &Selection<'_>, _capabilities: &FileCapabilities) -> Vec<MenuItem> {
        vec![item("newFolder", "新建文件夹…", "folder-plus", "newFolder")]
    }
}

/// Empty file creation is distinct from the VFS write capability used by
/// reviewed imports and transfers.
pub(super) struct NewFile;
impl NewFile {
    fn available(&self, selection: &Selection<'_>, capabilities: &FileCapabilities) -> bool {
        selection.paths.is_empty() && capabilities.write
    }
    fn items(&self, _selection: &Selection<'_>, _capabilities: &FileCapabilities) -> Vec<MenuItem> {
        vec![item("newFile", "新建空文件…", "file-plus", "newFile")]
    }
}

pub(super) struct Rename;
impl Rename {
    fn available(&self, selection: &Selection<'_>, capabilities: &FileCapabilities) -> bool {
        capabilities.rename && !selection.paths.is_empty()
    }
    fn items(&self, selection: &Selection<'_>, _: &FileCapabilities) -> Vec<MenuItem> {
        if selection.many() {
            Vec::new()
        } else {
            vec![item("rename", "重新命名", "pencil", "rename")]
        }
    }
}

pub(super) struct BatchRename;
impl BatchRename {
    fn available(&self, selection: &Selection<'_>, capabilities: &FileCapabilities) -> bool {
        capabilities.rename && !selection.paths.is_empty()
    }
    fn items(&self, selection: &Selection<'_>, _: &FileCapabilities) -> Vec<MenuItem> {
        if selection.many() {
            vec![item("batchRename", "批量重命名…", "pencil", "")]
        } else {
            Vec::new()
        }
    }
}

pub(super) struct Clipboard;
impl Clipboard {
    fn available(&self, selection: &Selection<'_>, capabilities: &FileCapabilities) -> bool {
        !selection.paths.is_empty() && capabilities.read
    }
    fn items(&self, _selection: &Selection<'_>, capabilities: &FileCapabilities) -> Vec<MenuItem> {
        let mut items = Vec::new();
        if capabilities.read {
            items.push(item("copy", "复制", "copy", "copy"));
        }
        if capabilities.read && capabilities.remove {
            items.push(item("cut", "剪切", "scissors", "cut"));
        }
        if capabilities.read && capabilities.write {
            items.push(item("duplicate", "创建副本", "copy", "duplicate"));
        }
        items
    }
}

pub(super) struct CopyPath;
impl CopyPath {
    fn available(&self, selection: &Selection<'_>, _: &FileCapabilities) -> bool {
        !selection.paths.is_empty()
    }
    fn items(&self, _: &Selection<'_>, _: &FileCapabilities) -> Vec<MenuItem> {
        vec![item("copyPath", "复制路径", "copy", "")]
    }
}

pub(super) struct Trash;
impl Trash {
    fn available(&self, selection: &Selection<'_>, capabilities: &FileCapabilities) -> bool {
        capabilities.remove && !selection.paths.is_empty()
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
    NewFolder,
    NewFile,
    Rename,
    BatchRename,
    Clipboard,
    CopyPath,
    Trash,
);
