//! Atomic actions whose semantics belong to a native view.
use super::NativeProviderInvocation;
use crate::core::volumes::{native_vfs, recycle, trash_vfs};
use fiex_provider_contracts::{
    ContextMenuProvider, MenuExecuteResult, MenuItem, MenuPlacement, ProviderError, ProviderInput,
    ProviderInvocation,
};

fn selection(ctx: &dyn ProviderInvocation) -> Result<(String, Vec<String>), ProviderError> {
    let directory = NativeProviderInvocation::directory(ctx)?.display.clone();
    let paths = (0..ctx.targets().len())
        .map(|index| {
            NativeProviderInvocation::resource(ctx, index).map(|resource| resource.display.clone())
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((directory, paths))
}

fn offered<'a>(
    ctx: &'a dyn ProviderInvocation,
    items: &[MenuItem],
) -> Result<&'a str, ProviderError> {
    let id = match ctx.input()? {
        ProviderInput::ContextMenuExecute(request) => request.item_id.as_deref(),
        _ => None,
    }
    .ok_or_else(|| ProviderError::invalid_params("menu execute requires an itemId"))?;
    if items.iter().any(|item| item.id == id && !item.disabled) {
        Ok(id)
    } else {
        Err(ProviderError::invalid_params(
            "native menu action was not offered",
        ))
    }
}

fn item(id: &str, title: String, icon: &str, danger: bool) -> MenuItem {
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

pub(super) struct Reveal;
impl ContextMenuProvider for Reveal {
    fn resolve(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<MenuItem>, ProviderError> {
        if ctx.method() != "contextMenu.resolve" {
            return Err(ProviderError::method_not_found(ctx.method()));
        }
        let (_, paths) = selection(ctx)?;
        let [path] = paths.as_slice() else {
            return Ok(Vec::new());
        };
        Ok(if native_vfs::recent_reveal_source(path).is_some() {
            vec![item(
                "reveal",
                crate::i18n::t("打开文件所在位置").into(),
                "reveal",
                false,
            )]
        } else {
            Vec::new()
        })
    }

    fn execute(&self, ctx: &dyn ProviderInvocation) -> Result<MenuExecuteResult, ProviderError> {
        if ctx.method() != "contextMenu.execute" {
            return Err(ProviderError::method_not_found(ctx.method()));
        }
        let (directory, paths) = selection(ctx)?;
        let id = offered(ctx, &self.resolve(&resolve_invocation(ctx)?)?)?;
        if id != "reveal"
            || paths
                .first()
                .and_then(|path| native_vfs::recent_reveal_source(path))
                .is_none()
        {
            return Err(ProviderError::invalid_params(
                "Recent source cannot be revealed",
            ));
        }
        Ok(core_action("reveal", paths, &directory))
    }
}

pub(super) struct Restore;
impl ContextMenuProvider for Restore {
    fn resolve(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<MenuItem>, ProviderError> {
        if ctx.method() != "contextMenu.resolve" {
            return Err(ProviderError::method_not_found(ctx.method()));
        }
        let (_, paths) = selection(ctx)?;
        if paths.len() != 1 {
            return Ok(Vec::new());
        }
        Ok(vec![item(
            "restore",
            crate::i18n::t("放回原处").into(),
            "restore",
            false,
        )])
    }

    fn execute(&self, ctx: &dyn ProviderInvocation) -> Result<MenuExecuteResult, ProviderError> {
        if ctx.method() != "contextMenu.execute" {
            return Err(ProviderError::method_not_found(ctx.method()));
        }
        let (directory, paths) = selection(ctx)?;
        let id = offered(ctx, &self.resolve(&resolve_invocation(ctx)?)?)?;
        if id != "restore" || paths.len() != 1 {
            return Err(ProviderError::invalid_params(
                "restore requires one Trash item",
            ));
        }
        Ok(core_action("restore", paths, &directory))
    }
}

pub(super) struct EmptyTrash;
impl ContextMenuProvider for EmptyTrash {
    fn resolve(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<MenuItem>, ProviderError> {
        if ctx.method() != "contextMenu.resolve" {
            return Err(ProviderError::method_not_found(ctx.method()));
        }
        let (directory, paths) = selection(ctx)?;
        if !paths.is_empty() || !matches!(directory.as_str(), trash_vfs::URI | recycle::URI) {
            return Ok(Vec::new());
        }
        Ok(vec![item(
            "emptyTrash",
            crate::i18n::t(if cfg!(windows) {
                "清空回收站"
            } else {
                "清空废纸篓"
            })
            .into(),
            "trash",
            true,
        )])
    }

    fn execute(&self, ctx: &dyn ProviderInvocation) -> Result<MenuExecuteResult, ProviderError> {
        if ctx.method() != "contextMenu.execute" {
            return Err(ProviderError::method_not_found(ctx.method()));
        }
        let (directory, paths) = selection(ctx)?;
        let id = offered(ctx, &self.resolve(&resolve_invocation(ctx)?)?)?;
        if id != "emptyTrash"
            || !paths.is_empty()
            || !matches!(directory.as_str(), trash_vfs::URI | recycle::URI)
        {
            return Err(ProviderError::invalid_params(
                "Empty Trash requires its root",
            ));
        }
        Ok(core_action("emptyTrash", paths, &directory))
    }
}

fn resolve_invocation(
    ctx: &dyn ProviderInvocation,
) -> Result<NativeProviderInvocation, ProviderError> {
    NativeProviderInvocation::context_menu_resolve(ctx)
}
