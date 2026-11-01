//! Native connection menus implement the shared connection-menu contract.
use super::Context;
use crate::plugins::{
    events::Resolver, native_invocation::NativeProviderInvocation,
    provider_registry::native::NativeProviderRegistry,
};
use fiex_provider_contracts::{
    ConnectionContextMenuProvider, MatchResource, MenuExecuteResult, MenuItem, MenuItemKind,
    MenuPlacement, ProviderError, ProviderInput, ProviderInvocation, ProviderKind,
    ProviderMatchRule,
};
use serde_json::{json, Value};
use std::sync::{Arc, OnceLock};

fn context(invocation: &dyn ProviderInvocation) -> Result<&Context, ProviderError> {
    NativeProviderInvocation::local_data(invocation)
}

#[cfg(test)]
pub(super) fn items(id: &str, ctx: &Context) -> Result<Vec<Value>, String> {
    let invocation =
        NativeProviderInvocation::new("connectionContextMenu.resolve", json!({}), &[], None)
            .with_local_data(Arc::new(ctx.clone()));
    let provider = registry()
        .get(id)
        .ok_or("unknown native connection menu provider")?;
    provider
        .resolve(&invocation)
        .map_err(|error| error.to_string())
        .and_then(|items| {
            serde_json::to_value(items)
                .map_err(|error| error.to_string())?
                .as_array()
                .cloned()
                .ok_or_else(|| "invalid connection menu result".to_string())
        })
}

fn available_action(
    provider: &dyn ConnectionContextMenuProvider,
    invocation: &dyn ProviderInvocation,
) -> Result<String, ProviderError> {
    let item_id = match invocation.input()? {
        ProviderInput::ConnectionMenuExecute(request) => request.item_id.as_deref(),
        _ => None,
    }
    .ok_or_else(|| ProviderError::invalid_params("missing menu action"))?;
    let offered_invocation =
        NativeProviderInvocation::new("connectionContextMenu.resolve", json!({}), &[], None)
            .with_local_data(Arc::new(context(invocation)?.clone()));
    let offered = provider.resolve(&offered_invocation)?;
    if !offered
        .iter()
        .any(|item| item.id == item_id && !item.disabled && item.kind == MenuItemKind::Action)
    {
        return Err(ProviderError::invalid_params(
            "menu action is no longer available",
        ));
    }
    Ok(item_id.to_owned())
}

fn item(id: &str, title: String, icon: &str) -> MenuItem {
    let mut item = MenuItem::action(id, title);
    item.icon = Some(icon.into());
    item
}

struct Edit;
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
        let item = available_action(self, invocation)?;
        if item != "connection.edit" {
            return Err(ProviderError::invalid_params(
                "unknown connection edit action",
            ));
        }
        Ok(MenuExecuteResult::native_action(item))
    }
}

struct Session;
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
        let item = available_action(self, invocation)?;
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

struct SyncMenu;
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
        let item = available_action(self, invocation)?;
        if item != "connection.syncNow" {
            return Err(ProviderError::invalid_params(
                "connection does not support sync action",
            ));
        }
        Ok(MenuExecuteResult::native_action(item))
    }
}

struct Close;
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
        let item = available_action(self, invocation)?;
        if item != "connection.close" {
            return Err(ProviderError::invalid_params(
                "unknown connection close action",
            ));
        }
        Ok(MenuExecuteResult::native_action(item))
    }
}

fn registry() -> &'static NativeProviderRegistry<dyn ConnectionContextMenuProvider> {
    static REGISTERED: OnceLock<NativeProviderRegistry<dyn ConnectionContextMenuProvider>> =
        OnceLock::new();
    REGISTERED.get_or_init(|| {
        let mut values: NativeProviderRegistry<dyn ConnectionContextMenuProvider> =
            NativeProviderRegistry::new(ProviderKind::ConnectionContextMenu);
        let implementations: [(&str, i64, Arc<dyn ConnectionContextMenuProvider>); 4] = [
            (
                "builtin:connection-edit",
                100,
                Arc::new(Edit) as Arc<dyn ConnectionContextMenuProvider>,
            ),
            (
                "builtin:connection-session",
                90,
                Arc::new(Session) as Arc<dyn ConnectionContextMenuProvider>,
            ),
            (
                "builtin:connection-sync",
                80,
                Arc::new(SyncMenu) as Arc<dyn ConnectionContextMenuProvider>,
            ),
            (
                "builtin:connection-close",
                70,
                Arc::new(Close) as Arc<dyn ConnectionContextMenuProvider>,
            ),
        ];
        for (id, priority, provider) in implementations {
            values
                .register(id, priority, ProviderMatchRule::default(), provider)
                .expect("invalid native connection-menu registration");
        }
        values
    })
}

pub(super) fn providers(targets: &[MatchResource]) -> Vec<Resolver> {
    registry().matched_candidates(targets, None, targets.len())
}

pub(super) fn provider(id: &str) -> Option<Arc<dyn ConnectionContextMenuProvider>> {
    registry().get_arc(id)
}

pub(super) fn invoke(id: &str, invocation: &dyn ProviderInvocation) -> Result<Value, String> {
    let provider = registry()
        .get(id)
        .ok_or("unknown native connection menu provider")?;
    match invocation.method() {
        "connectionContextMenu.resolve" => provider
            .resolve(invocation)
            .map(|items| json!({"items":items}))
            .map_err(|error| error.to_string()),
        "connectionContextMenu.execute" => provider
            .execute(invocation)
            .and_then(|result| {
                serde_json::to_value(result)
                    .map_err(|error| ProviderError::internal(error.to_string()))
            })
            .map_err(|error| error.to_string()),
        method => Err(ProviderError::method_not_found(method).to_string()),
    }
}

/// Run an authorized native action after the shared provider revalidates it.
pub(super) async fn execute(
    provider_id: &str,
    ctx: &Context,
    item_id: &str,
) -> Result<MenuExecuteResult, String> {
    let invocation = NativeProviderInvocation::new(
        "connectionContextMenu.execute",
        json!({"itemId":item_id}),
        &[],
        None,
    )
    .with_local_data(Arc::new(ctx.clone()));
    let result = registry()
        .get(provider_id)
        .ok_or("unknown native connection menu provider")?
        .execute(&invocation)
        .map_err(|error| error.to_string())?;
    if !matches!(&result, MenuExecuteResult::NativeAction(action) if action.item_id == item_id) {
        return Err("invalid native connection menu action".into());
    }
    match item_id {
        "connection.edit" => Ok(MenuExecuteResult::connection_core_action(
            "editConnection",
            ctx.info.id.clone(),
        )),
        "connection.close" => Ok(MenuExecuteResult::connection_core_action(
            "closeConnection",
            ctx.info.id.clone(),
        )),
        "connection.saveSession" => {
            crate::remote::update_connection_session(&ctx.info.id, Some(true), None)?;
            Ok(MenuExecuteResult::handled())
        }
        "connection.toggleAutostart" => {
            crate::remote::update_connection_session(
                &ctx.info.id,
                None,
                Some(!ctx.info.autostart),
            )?;
            Ok(MenuExecuteResult::handled())
        }
        "connection.syncNow" => {
            crate::remote::reconcile_connection(&ctx.info.id).await;
            Ok(MenuExecuteResult::handled())
        }
        _ => Err("unknown native connection menu action".into()),
    }
}

#[cfg(test)]
fn registry_len() -> usize {
    registry().len()
}

#[cfg(test)]
fn descriptors() -> impl Iterator<Item = &'static fiex_provider_contracts::ProviderDescriptor> {
    registry().descriptors()
}

#[cfg(test)]
fn get(id: &str) -> Option<&'static dyn ConnectionContextMenuProvider> {
    registry().get(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_connections_use_exact_shared_provider_descriptors() {
        assert_eq!(registry_len(), 4);
        for descriptor in descriptors() {
            assert_eq!(descriptor.kind, ProviderKind::ConnectionContextMenu);
            descriptor.validate().unwrap();
        }
        let target = MatchResource {
            metadata: Some(fiex_provider_contracts::ResourceMetadata {
                kind: "connection".into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let candidates = providers(&[target]);
        assert_eq!(candidates.len(), 4);
        assert_eq!(candidates[0].id, "builtin:connection-edit");
        assert_eq!(candidates[3].id, "builtin:connection-close");
        assert!(get("builtin:unregistered").is_none());
    }
}
