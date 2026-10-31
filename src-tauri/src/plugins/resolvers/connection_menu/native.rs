//! Core connection menus implement one typed resolve/execute contract.
//! Dispatch is direct for native providers; plugin actions remain IPC.
use super::Context;
use crate::plugins::events::Resolver;
use crate::plugins::provider_registry::native::NativeProviderRegistry;
use fiex_provider_contracts::{ProviderKind, ProviderMatchRule};
use serde_json::{json, Value};
use std::sync::{Arc, OnceLock};

enum NativeEffect {
    CoreAction(&'static str),
    SaveSession,
    ToggleAutostart,
    SyncNow,
}

trait NativeConnectionMenu: Send + Sync {
    fn items(&self, ctx: &Context) -> Vec<Value>;
    fn execute(&self, ctx: &Context, item: &str) -> Result<NativeEffect, String>;
}

fn item(id: &str, title: String, icon: &str) -> Value {
    json!({"id":id,"kind":"action","title":title,"icon":icon})
}

struct Edit;
impl NativeConnectionMenu for Edit {
    fn items(&self, _ctx: &Context) -> Vec<Value> {
        vec![item(
            "connection.edit",
            crate::i18n::t("编辑连接…").into(),
            "pencil",
        )]
    }

    fn execute(&self, _ctx: &Context, item: &str) -> Result<NativeEffect, String> {
        match item {
            "connection.edit" => Ok(NativeEffect::CoreAction("editConnection")),
            _ => Err("unknown connection edit action".into()),
        }
    }
}

struct Session;
impl NativeConnectionMenu for Session {
    fn items(&self, ctx: &Context) -> Vec<Value> {
        vec![
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
        ]
    }

    fn execute(&self, _ctx: &Context, item: &str) -> Result<NativeEffect, String> {
        match item {
            "connection.saveSession" => Ok(NativeEffect::SaveSession),
            "connection.toggleAutostart" => Ok(NativeEffect::ToggleAutostart),
            _ => Err("unknown connection session action".into()),
        }
    }
}

struct SyncMenu;
impl NativeConnectionMenu for SyncMenu {
    fn items(&self, ctx: &Context) -> Vec<Value> {
        if ctx.can_sync {
            vec![item(
                "connection.syncNow",
                crate::i18n::t("立即同步").into(),
                "sync",
            )]
        } else {
            Vec::new()
        }
    }

    fn execute(&self, ctx: &Context, item: &str) -> Result<NativeEffect, String> {
        if item == "connection.syncNow" && ctx.can_sync {
            Ok(NativeEffect::SyncNow)
        } else {
            Err("connection does not support sync action".into())
        }
    }
}

struct Close;
impl NativeConnectionMenu for Close {
    fn items(&self, _ctx: &Context) -> Vec<Value> {
        let mut close = item(
            "connection.close",
            crate::i18n::t("关闭连接").into(),
            "trash",
        );
        close["danger"] = json!(true);
        close["placement"] = json!("tail");
        vec![close]
    }

    fn execute(&self, _ctx: &Context, item: &str) -> Result<NativeEffect, String> {
        match item {
            "connection.close" => Ok(NativeEffect::CoreAction("closeConnection")),
            _ => Err("unknown connection close action".into()),
        }
    }
}

fn registry() -> &'static NativeProviderRegistry<dyn NativeConnectionMenu> {
    static REGISTERED: OnceLock<NativeProviderRegistry<dyn NativeConnectionMenu>> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let mut values: NativeProviderRegistry<dyn NativeConnectionMenu> =
            NativeProviderRegistry::new(ProviderKind::ConnectionContextMenu);
        let implementations: [(&str, i64, Arc<dyn NativeConnectionMenu>); 4] = [
            ("builtin:connection-edit", 100, Arc::new(Edit)),
            ("builtin:connection-session", 90, Arc::new(Session)),
            ("builtin:connection-sync", 80, Arc::new(SyncMenu)),
            ("builtin:connection-close", 70, Arc::new(Close)),
        ];
        for (id, priority, provider) in implementations {
            values
                .register(id, priority, ProviderMatchRule::default(), provider)
                .expect("invalid native connection-menu registration");
        }
        values
    })
}

pub(super) fn providers() -> Vec<Resolver> {
    registry().candidates(|_| true)
}

pub(super) fn items(id: &str, ctx: &Context) -> Result<Vec<Value>, String> {
    registry()
        .get(id)
        .map(|provider| provider.items(ctx))
        .ok_or_else(|| "unknown native connection menu provider".into())
}

/// Execute an action on its own registered native provider. The Core action
/// boundary stays async for session mutations and synchronization.
pub(super) async fn execute(
    provider_id: &str,
    ctx: &Context,
    item_id: &str,
) -> Result<Value, String> {
    let provider = registry()
        .get(provider_id)
        .ok_or("unknown native connection menu provider")?;
    if !provider.items(ctx).iter().any(|value| {
        value["id"] == item_id && value["kind"] == "action" && value["disabled"] != true
    }) {
        return Err("native menu action is not available".into());
    }
    let effect = provider.execute(ctx, item_id)?;
    match effect {
        NativeEffect::CoreAction(command) => Ok(json!({
            "kind":"coreAction",
            "command":command,
            "connectionId":ctx.info.id
        })),
        NativeEffect::SaveSession => {
            crate::remote::update_connection_session(&ctx.info.id, Some(true), None)?;
            Ok(json!({"kind":"handled"}))
        }
        NativeEffect::ToggleAutostart => {
            crate::remote::update_connection_session(
                &ctx.info.id,
                None,
                Some(!ctx.info.autostart),
            )?;
            Ok(json!({"kind":"handled"}))
        }
        NativeEffect::SyncNow => {
            crate::remote::reconcile_connection(&ctx.info.id).await;
            Ok(json!({"kind":"handled"}))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_connections_use_exact_provider_descriptors() {
        assert_eq!(registry().len(), 4);
        for descriptor in registry().descriptors() {
            assert_eq!(descriptor.kind, ProviderKind::ConnectionContextMenu);
            descriptor.validate().unwrap();
        }
        let candidates = providers();
        assert_eq!(candidates.len(), 4);
        assert_eq!(candidates[0].id, "builtin:connection-edit");
        assert_eq!(candidates[3].id, "builtin:connection-close");
        assert!(registry().get("builtin:unregistered").is_none());
    }
}
