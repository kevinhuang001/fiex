use super::super::{Context, Selection};
use super::{item, new_file::new_file_items};
use crate::plugins::native_invocation::NativeProviderInvocation;
use fiex_provider_contracts::{
    ContextMenuProvider, FsOperation, MenuExecuteResult, MenuItem, MenuPlacement, ProviderError,
    ProviderInput, ProviderInvocation,
};
use fiex_resource::FileCapabilities;
use std::sync::Arc;

use fiex_actions::Action;

pub(super) struct AtomicMenuAction(pub(super) Action);

fn can_copy_sources(selection: &Selection<'_>) -> bool {
    !selection.paths.is_empty()
        && selection.paths.len() == selection.entries.len()
        && selection.paths.len() == selection.entry_capabilities.len()
        && selection
            .entries
            .iter()
            .zip(selection.entry_capabilities)
            .all(|(entry, capabilities)| {
                capabilities.allows(if entry.is_dir {
                    FsOperation::List
                } else {
                    FsOperation::Read
                })
            })
}

fn can_cut_sources(selection: &Selection<'_>) -> bool {
    can_copy_sources(selection)
        && selection
            .entry_capabilities
            .iter()
            .all(|capabilities| capabilities.remove)
}

fn can_duplicate_to_directory(selection: &Selection<'_>) -> bool {
    let needs_folder = selection.entries.iter().any(|entry| entry.is_dir);
    let needs_file = selection.entries.iter().any(|entry| !entry.is_dir);
    (!needs_folder || selection.directory_capabilities.create)
        && (!needs_file || selection.directory_capabilities.write)
}

fn can_open(selection: &Selection<'_>, capabilities: &FileCapabilities) -> bool {
    !selection.paths.is_empty()
        && selection.paths.iter().enumerate().all(|(index, path)| {
            let Some(entry) = selection.entries.get(index) else {
                return false;
            };
            crate::core::volumes::native_vfs::open_policy(path, entry)
                .unwrap_or_else(|| entry.is_dir || capabilities.read || entry.link_target.is_some())
        })
}

impl AtomicMenuAction {
    fn items(&self, selection: &Selection<'_>, caps: &FileCapabilities) -> Vec<MenuItem> {
        let nonzero = !selection.paths.is_empty();
        let one = selection.paths.len() == 1;
        let mut items = match self.0 {
            Action::Open if can_open(selection, caps) => {
                vec![item("open", "打开", "open", "open")]
            }
            Action::OpenWith if one && caps.read => {
                vec![item(
                    "openWith-choose",
                    "打开方式",
                    "application-search",
                    "",
                )]
            }
            Action::Preview if one && caps.read => {
                vec![item("preview", "快速查看", "eye", "quickLook")]
            }
            Action::Info if one => vec![item("info", "显示简介", "info", "info")],
            Action::Tags if selection.all_local && cfg!(target_os = "macos") && nonzero => {
                vec![item("tags", "标签…", "tags", "")]
            }
            Action::NewFolder
                if selection.paths.is_empty() && caps.create && selection.preflight.new_folder =>
            {
                vec![item("newFolder", "新建文件夹…", "folder-plus", "newFolder")]
            }
            Action::NewFile
                if selection.paths.is_empty() && caps.write && selection.preflight.new_file =>
            {
                new_file_items()
            }
            Action::Rename if one && caps.rename && selection.preflight.rename => {
                vec![item("rename", "重新命名", "pencil", "rename")]
            }
            Action::BatchRename
                if selection.many() && caps.rename && selection.preflight.batch_rename =>
            {
                vec![item("batchRename", "批量重命名…", "pencil", "")]
            }
            Action::Copy if can_copy_sources(selection) => {
                vec![item("copy", "复制", "copy", "copy")]
            }
            // Cut records references only. Import/Export/Move authorization is
            // checked when Paste or drag-and-drop actually performs a transfer.
            Action::Cut if can_cut_sources(selection) => {
                vec![item("cut", "剪切", "scissors", "cut")]
            }
            Action::Duplicate
                if can_copy_sources(selection) && can_duplicate_to_directory(selection) =>
            {
                vec![item("duplicate", "创建副本", "copy", "duplicate")]
            }
            Action::CopyPath if nonzero => vec![item("copyPath", "复制路径", "copy", "")],
            Action::MoveToTrash if nonzero && caps.trash && selection.preflight.trash => {
                vec![item(
                    "trash",
                    if cfg!(windows) {
                        "移到回收站"
                    } else {
                        "移到废纸篓"
                    },
                    "trash",
                    "trash",
                )]
            }
            Action::DeletePermanently if nonzero && caps.remove && selection.preflight.remove => {
                vec![item("delete", "彻底删除", "trash", "")]
            }
            Action::Pin
            | Action::Unpin
            | Action::Cache
            | Action::Uncache
            | Action::ConflictResolution => {
                if !nonzero
                    || !selection
                        .paths
                        .iter()
                        .all(|path| crate::remote::is_remote(path))
                {
                    return Vec::new();
                }
                let wanted = match self.0 {
                    Action::Pin => "remote.pin",
                    Action::Unpin => "remote.unpin",
                    Action::Cache => "remote.cache",
                    Action::Uncache => "remote.uncache",
                    Action::ConflictResolution => "",
                    _ => unreachable!(),
                };
                let mut items =
                    super::super::super::connection_cache::ConnectionCache.items(selection, caps);
                if self.0 == Action::ConflictResolution {
                    items.retain(|item| {
                        matches!(item.id.as_str(), "remote.keepLocal" | "remote.keepRemote")
                    });
                } else {
                    items.retain(|item| item.id == wanted);
                }
                items
            }
            _ => Vec::new(),
        };
        for item in &mut items {
            if matches!(self.0, Action::MoveToTrash | Action::DeletePermanently) {
                item.placement = Some(MenuPlacement::Tail);
                item.danger = true;
            }
        }
        items
    }
}

impl ContextMenuProvider for AtomicMenuAction {
    fn resolve(&self, invocation: &dyn ProviderInvocation) -> Result<Vec<MenuItem>, ProviderError> {
        if invocation.method() != "contextMenu.resolve" {
            return Err(ProviderError::method_not_found(invocation.method()));
        }
        let context = NativeProviderInvocation::local_data::<Context>(invocation)?;
        Ok(self.items(&context.selection(), &context.capabilities()))
    }

    fn execute(
        &self,
        invocation: &dyn ProviderInvocation,
    ) -> Result<MenuExecuteResult, ProviderError> {
        if invocation.method() != "contextMenu.execute" {
            return Err(ProviderError::method_not_found(invocation.method()));
        }
        let context = NativeProviderInvocation::local_data::<Context>(invocation)?;
        let (item_id, menu_session_id) = match invocation.input()? {
            ProviderInput::ContextMenuExecute(request) => (
                request.item_id.as_deref(),
                request.menu_session_id.as_deref(),
            ),
            _ => (None, None),
        };
        let item_id = item_id
            .ok_or_else(|| ProviderError::invalid_params("menu execute requires an itemId"))?;
        let _menu_session_id = menu_session_id
            .ok_or_else(|| ProviderError::invalid_params("menu session is missing"))?;
        let resolve = NativeProviderInvocation::context_menu_resolve(invocation)?
            .with_local_data(Arc::new(context.clone()));
        let offered = self.resolve(&resolve)?;
        if !offered
            .iter()
            .any(|item| item.id == item_id && !item.disabled)
        {
            return Err(ProviderError::invalid_params(
                "native menu action was not offered",
            ));
        }
        if self.0 == Action::OpenWith {
            return Ok(MenuExecuteResult::core_action(
                "chooseOpenWith",
                context.paths.clone(),
                context.paths.first().cloned().unwrap_or_default(),
                context.directory.clone(),
            ));
        }
        Ok(super::super::super::builtin::action_result(
            item_id, context,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selection<'a>(
        paths: &'a [String],
        preflight: super::super::super::super::builtin::MenuPreflightAvailability,
    ) -> Selection<'a> {
        Selection {
            paths,
            entries: &[],
            entry_capabilities: &[],
            directory_capabilities: FileCapabilities::default(),
            all_local: true,
            preflight,
        }
    }

    #[test]
    fn trash_and_remove_capabilities_are_independent_and_coexist() {
        let paths = ["/tmp/item".to_owned()];
        let selection = selection(
            &paths,
            super::super::super::super::builtin::MenuPreflightAvailability {
                trash: true,
                remove: true,
                ..Default::default()
            },
        );
        for (trash, remove, expected) in [
            (false, false, [false, false]),
            (true, false, [true, false]),
            (false, true, [false, true]),
            (true, true, [true, true]),
        ] {
            let caps = FileCapabilities {
                trash,
                remove,
                ..Default::default()
            };
            assert_eq!(
                [
                    !AtomicMenuAction(Action::MoveToTrash)
                        .items(&selection, &caps)
                        .is_empty(),
                    !AtomicMenuAction(Action::DeletePermanently)
                        .items(&selection, &caps)
                        .is_empty(),
                ],
                expected
            );
        }
    }
}
