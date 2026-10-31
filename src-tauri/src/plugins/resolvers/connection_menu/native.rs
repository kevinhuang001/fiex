//! Core connection-menu actions use persistent, typed providers. Each
//! implementation owns its own menu policy; IPC proxies share the same
//! EventRouter and response validation.
use super::Context;
use crate::plugins::events::Resolver;
use crate::plugins::provider_registry::native::NativeProviderRegistry;
use fiex_provider_contracts::{ProviderKind, ProviderMatchRule};
use serde_json::{json, Value};
use std::sync::{Arc, OnceLock};

trait NativeConnectionMenu: Send + Sync {
    fn items(&self, ctx: &Context) -> Vec<Value>;
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
}

struct Sync;
impl NativeConnectionMenu for Sync {
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
}

fn registry() -> &'static NativeProviderRegistry<dyn NativeConnectionMenu> {
    static REGISTERED: OnceLock<NativeProviderRegistry<dyn NativeConnectionMenu>> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let mut values: NativeProviderRegistry<dyn NativeConnectionMenu> =
            NativeProviderRegistry::new(ProviderKind::ConnectionContextMenu);
        let implementations: [(&str, i64, Arc<dyn NativeConnectionMenu>); 4] = [
            ("builtin:connection-edit", 100, Arc::new(Edit)),
            ("builtin:connection-session", 90, Arc::new(Session)),
            ("builtin:connection-sync", 80, Arc::new(Sync)),
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
