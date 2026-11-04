use super::{available_action, context};
use fiex_provider_contracts::{
    ConnectionContextMenuProvider, MenuExecuteResult, MenuItem, MenuPlacement, ProviderError,
    ProviderInvocation,
};

fn item(id: &str, title: String, icon: &str) -> MenuItem {
    let mut item = MenuItem::action(id, title);
    item.icon = Some(icon.into());
    item
}

pub(super) struct Edit;
impl ConnectionContextMenuProvider for Edit {
    fn resolve(&self, invocation: &dyn ProviderInvocation) -> Result<Vec<MenuItem>, ProviderError> {
        if invocation.method() != "connectionContextMenu.resolve" {
            return Err(ProviderError::method_not_found(invocation.method()));
        }
        context(invocation)?;
        Ok(vec![item(
            "connection.edit",
            crate::i18n::t("编辑连接…").into(),
            "pencil",
        )])
    }

    fn execute(
        &self,
        invocation: &dyn ProviderInvocation,
    ) -> Result<MenuExecuteResult, ProviderError> {
        if invocation.method() != "connectionContextMenu.execute" {
            return Err(ProviderError::method_not_found(invocation.method()));
        }
        let item = available_action("builtin:connection-edit", self, invocation)?;
        if item != "connection.edit" {
            return Err(ProviderError::invalid_params(
                "unknown connection edit action",
            ));
        }
        Ok(MenuExecuteResult::native_action(item))
    }
}

pub(super) struct Session;
impl ConnectionContextMenuProvider for Session {
    fn resolve(&self, invocation: &dyn ProviderInvocation) -> Result<Vec<MenuItem>, ProviderError> {
        if invocation.method() != "connectionContextMenu.resolve" {
            return Err(ProviderError::method_not_found(invocation.method()));
        }
        let ctx = context(invocation)?;
        Ok(vec![
            item(
                "connection.saveSession",
                crate::i18n::t("保存 Session").into(),
                "pin",
            ),
            item(
                "connection.toggleAutostart",
                if ctx.info.autostart {
                    crate::i18n::t("取消自启动").into()
                } else {
                    crate::i18n::t("设置为自启动").into()
                },
                "pin",
            ),
        ])
    }

    fn execute(
        &self,
        invocation: &dyn ProviderInvocation,
    ) -> Result<MenuExecuteResult, ProviderError> {
        if invocation.method() != "connectionContextMenu.execute" {
            return Err(ProviderError::method_not_found(invocation.method()));
        }
        let item = available_action("builtin:connection-session", self, invocation)?;
        if !matches!(
            item.as_str(),
            "connection.saveSession" | "connection.toggleAutostart"
        ) {
            return Err(ProviderError::invalid_params(
                "unknown connection session action",
            ));
        }
        Ok(MenuExecuteResult::native_action(item))
    }
}

pub(super) struct SyncMenu;
impl ConnectionContextMenuProvider for SyncMenu {
    fn resolve(&self, invocation: &dyn ProviderInvocation) -> Result<Vec<MenuItem>, ProviderError> {
        if invocation.method() != "connectionContextMenu.resolve" {
            return Err(ProviderError::method_not_found(invocation.method()));
        }
        if context(invocation)?.can_sync {
            Ok(vec![item(
                "connection.syncNow",
                crate::i18n::t("立即同步").into(),
                "sync",
            )])
        } else {
            Ok(Vec::new())
        }
    }

    fn execute(
        &self,
        invocation: &dyn ProviderInvocation,
    ) -> Result<MenuExecuteResult, ProviderError> {
        if invocation.method() != "connectionContextMenu.execute" {
            return Err(ProviderError::method_not_found(invocation.method()));
        }
        let item = available_action("builtin:connection-sync", self, invocation)?;
        if item != "connection.syncNow" {
            return Err(ProviderError::invalid_params(
                "connection does not support sync action",
            ));
        }
        Ok(MenuExecuteResult::native_action(item))
    }
}

pub(super) struct Close;
impl ConnectionContextMenuProvider for Close {
    fn resolve(&self, invocation: &dyn ProviderInvocation) -> Result<Vec<MenuItem>, ProviderError> {
        if invocation.method() != "connectionContextMenu.resolve" {
            return Err(ProviderError::method_not_found(invocation.method()));
        }
        context(invocation)?;
        let mut close = item(
            "connection.close",
            crate::i18n::t("关闭连接").into(),
            "trash",
        );
        close.danger = true;
        close.placement = Some(MenuPlacement::Tail);
        Ok(vec![close])
    }

    fn execute(
        &self,
        invocation: &dyn ProviderInvocation,
    ) -> Result<MenuExecuteResult, ProviderError> {
        if invocation.method() != "connectionContextMenu.execute" {
            return Err(ProviderError::method_not_found(invocation.method()));
        }
        let item = available_action("builtin:connection-close", self, invocation)?;
        if item != "connection.close" {
            return Err(ProviderError::invalid_params(
                "unknown connection close action",
            ));
        }
        Ok(MenuExecuteResult::native_action(item))
    }
}
