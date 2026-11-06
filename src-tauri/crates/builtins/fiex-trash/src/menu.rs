use crate::{TrashModuleServices, RECYCLE_BIN_URI, TRASH_URI};
use fiex_provider_contracts::{
    ContextMenuProvider, MenuExecuteResult, MenuItem, MenuPlacement, ProviderError, ProviderInput,
    ProviderInvocation,
};
use std::sync::Arc;

pub(super) struct RestoreMenu {
    services: Arc<dyn TrashModuleServices>,
}

impl RestoreMenu {
    pub(super) fn new(services: Arc<dyn TrashModuleServices>) -> Self {
        Self { services }
    }
}

impl ContextMenuProvider for RestoreMenu {
    fn resolve(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<MenuItem>, ProviderError> {
        if ctx.method() != "contextMenu.resolve" {
            return Err(ProviderError::method_not_found(ctx.method()));
        }
        let (_, paths) = self.services.selection(ctx)?;
        if paths.len() != 1 {
            return Ok(Vec::new());
        }
        Ok(vec![action_item(
            "restore",
            self.services.translate("放回原处"),
            "restore",
            false,
        )])
    }

    fn execute(&self, ctx: &dyn ProviderInvocation) -> Result<MenuExecuteResult, ProviderError> {
        if ctx.method() != "contextMenu.execute" {
            return Err(ProviderError::method_not_found(ctx.method()));
        }
        let (directory, paths) = self.services.selection(ctx)?;
        if offered_item(ctx)? != "restore" || paths.len() != 1 {
            return Err(ProviderError::invalid_params(
                "restore requires one Trash item",
            ));
        }
        Ok(core_action("restore", paths, &directory))
    }
}

pub(super) struct EmptyTrashMenu {
    services: Arc<dyn TrashModuleServices>,
}

impl EmptyTrashMenu {
    pub(super) fn new(services: Arc<dyn TrashModuleServices>) -> Self {
        Self { services }
    }
}

impl ContextMenuProvider for EmptyTrashMenu {
    fn resolve(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<MenuItem>, ProviderError> {
        if ctx.method() != "contextMenu.resolve" {
            return Err(ProviderError::method_not_found(ctx.method()));
        }
        let (directory, paths) = self.services.selection(ctx)?;
        if !paths.is_empty() || !is_root(&directory) {
            return Ok(Vec::new());
        }
        let title = if cfg!(windows) {
            "清空回收站"
        } else {
            "清空废纸篓"
        };
        Ok(vec![action_item(
            "emptyTrash",
            self.services.translate(title),
            "trash",
            true,
        )])
    }

    fn execute(&self, ctx: &dyn ProviderInvocation) -> Result<MenuExecuteResult, ProviderError> {
        if ctx.method() != "contextMenu.execute" {
            return Err(ProviderError::method_not_found(ctx.method()));
        }
        let (directory, paths) = self.services.selection(ctx)?;
        if offered_item(ctx)? != "emptyTrash" || !paths.is_empty() || !is_root(&directory) {
            return Err(ProviderError::invalid_params(
                "Empty Trash requires its root",
            ));
        }
        Ok(core_action("emptyTrash", paths, &directory))
    }
}

fn is_root(uri: &str) -> bool {
    uri == TRASH_URI || uri == RECYCLE_BIN_URI
}

fn offered_item(ctx: &dyn ProviderInvocation) -> Result<&str, ProviderError> {
    match ctx.input()? {
        ProviderInput::ContextMenuExecute(request) => request
            .item_id
            .as_deref()
            .ok_or_else(|| ProviderError::invalid_params("menu execute requires an itemId")),
        _ => Err(ProviderError::invalid_params(
            "context menu execution input is required",
        )),
    }
}

fn action_item(id: &str, title: String, icon: &str, danger: bool) -> MenuItem {
    let mut item = MenuItem::action(id, title);
    item.icon = Some(icon.into());
    item.danger = danger;
    if danger {
        item.placement = Some(MenuPlacement::Tail);
    }
    item
}

fn core_action(command: &str, paths: Vec<String>, directory: &str) -> MenuExecuteResult {
    let path = paths.first().cloned().unwrap_or_default();
    MenuExecuteResult::core_action(command, paths, path, directory)
}
