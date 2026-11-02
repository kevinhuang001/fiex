//! Native connection menus implement the shared connection-menu contract.
use self::providers::{Close, Edit, Session, SyncMenu};
use super::Context;
use crate::plugins::{
    events::Resolver, native_invocation::NativeProviderInvocation,
    provider_registry::native::NativeProviderRegistry,
};
use fiex_provider_contracts::{
    ConnectionContextMenuProvider, MatchResource, MenuExecuteResult, MenuItemKind, ProviderError,
    ProviderInput, ProviderInvocation, ProviderKind, ProviderMatchRule,
};
use serde_json::json;

mod providers;
#[cfg(test)]
use serde_json::Value;
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
mod tests;
