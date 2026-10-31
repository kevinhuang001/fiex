//! Connection context menus: builtin providers plus plugin registrations.
//!
//! The connection is the menu target, not a file selection. Builtin providers
//! own the shell actions (edit, session, sync, close); plugins declaring
//! `extensions.connectionContextMenus` add their own flat action list.

use crate::plugins::events::{EventRouter, Handler, Resolver};
use crate::remote::ConnectionInfo;
use serde_json::{json, Value};

const BUILTIN_PREFIX: &str = "builtin:";
const PLUGIN_PREFIX: &str = "plugin:";

struct Context {
    info: ConnectionInfo,
    can_sync: bool,
}

impl Context {
    fn load(id: &str) -> Result<Self, String> {
        let info = crate::remote::list_connections()
            .into_iter()
            .find(|connection| connection.id == id)
            .ok_or("connection not found")?;
        let plugin_view = info.mount_path.starts_with("fiex-plugin-view://");
        let can_sync = !plugin_view && info.mount_path.starts_with("fiex://mount/");
        Ok(Self { info, can_sync })
    }

    /// The untrusted plugin sees identity and capabilities, never credentials.
    fn summary(&self) -> Value {
        json!({
            "id": self.info.id,
            "name": self.info.name,
            "kind": self.info.kind,
            "pluginId": self.info.plugin_id,
            "mountPath": self.info.mount_path,
            "saved": self.info.saved,
            "autostart": self.info.autostart,
            "pluginView": self.info.mount_path.starts_with("fiex-plugin-view://"),
            "vfs": self.can_sync,
        })
    }
}

fn providers() -> Vec<Resolver> {
    vec![
        Resolver::builtin("builtin:connection-edit", 100),
        Resolver::builtin("builtin:connection-session", 90),
        Resolver::builtin("builtin:connection-sync", 80),
        Resolver::builtin("builtin:connection-close", 70),
    ]
}

fn item(id: &str, title: String, icon: &str) -> Value {
    json!({"id":id,"kind":"action","title":title,"icon":icon})
}

fn builtin_items(id: &str, ctx: &Context) -> Result<Vec<Value>, String> {
    match id {
        "builtin:connection-edit" => Ok(vec![item(
            "connection.edit",
            crate::i18n::t("编辑连接…").into(),
            "pencil",
        )]),
        "builtin:connection-session" => {
            let mut items = vec![item(
                "connection.saveSession",
                crate::i18n::t("保存 Session").into(),
                "pin",
            )];
            items.push(item(
                "connection.toggleAutostart",
                if ctx.info.autostart {
                    crate::i18n::t("取消自启动").into()
                } else {
                    crate::i18n::t("设置为自启动").into()
                },
                "pin",
            ));
            Ok(items)
        }
        "builtin:connection-sync" => Ok(if ctx.can_sync {
            vec![item(
                "connection.syncNow",
                crate::i18n::t("立即同步").into(),
                "sync",
            )]
        } else {
            Vec::new()
        }),
        "builtin:connection-close" => {
            let mut close = item(
                "connection.close",
                crate::i18n::t("关闭连接").into(),
                "trash",
            );
            close["danger"] = json!(true);
            close["placement"] = json!("tail");
            Ok(vec![close])
        }
        _ => Err("unknown connection menu provider".into()),
    }
}

fn valid_items(value: &Value) -> Result<Vec<Value>, String> {
    let items = value
        .get("items")
        .and_then(Value::as_array)
        .ok_or("menu response requires items[]")?;
    let mut seen = std::collections::HashSet::new();
    for item in items {
        let id = item
            .get("id")
            .and_then(Value::as_str)
            .ok_or("menu item id missing")?;
        if id.is_empty()
            || id.len() > 80
            || !id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
            || !seen.insert(id.to_owned())
        {
            return Err("invalid or duplicate menu item id".into());
        }
        if !matches!(
            item.get("kind").and_then(Value::as_str),
            Some("action" | "separator")
        ) {
            return Err("invalid connection menu item kind".into());
        }
        if item.get("kind").and_then(Value::as_str) == Some("action") {
            let title = item
                .get("title")
                .and_then(Value::as_str)
                .ok_or("menu item title missing")?;
            if title.is_empty() || title.len() > 128 {
                return Err("invalid menu item title".into());
            }
        }
        if item.get("parentId").is_some() {
            return Err("connection menu items must be flat".into());
        }
    }
    Ok(items.clone())
}

pub fn query(connection_id: &str) -> Result<Vec<Value>, String> {
    let ctx = Context::load(connection_id)?;
    let session = crate::plugins::mounts::new_id()?;
    let providers = crate::plugins::provider_registry::candidates("connectionContextMenu.resolve", &[], "", providers());
    let mut menus = Vec::new();
    for provider in providers {
        let result = match &provider.handler {
            Handler::Builtin(id) => builtin_items(id, &ctx).and_then(|items| {
                let value = json!({"items":items});
                valid_items(&value)
            }),
            Handler::Plugin(plugin) => plugin_items(plugin, &ctx, &session),
        };
        let Ok(items) = result else {
            continue;
        };
        if items.is_empty() {
            continue;
        }
        let menu = match &provider.handler {
            Handler::Builtin(_) => json!({
                "kind": "builtin",
                "providerId": provider.id,
                "menuSessionId": session,
                "items": items,
            }),
            Handler::Plugin(plugin) => json!({
                "kind": "plugin",
                "providerId": provider.id,
                "pluginId": plugin.id,
                "pluginName": plugin.name,
                "menuSessionId": session,
                "items": items,
            }),
        };
        menus.push(menu);
    }
    Ok(menus)
}

pub async fn execute(
    connection_id: &str,
    provider_id: &str,
    item_id: &str,
    menu_session_id: Option<&str>,
) -> Result<Value, String> {
    let ctx = Context::load(connection_id)?;
    if let Some(id) = provider_id.strip_prefix(BUILTIN_PREFIX) {
        let provider = format!("{BUILTIN_PREFIX}{id}");
        let items = builtin_items(&provider, &ctx)?;
        if !items.iter().any(|item| {
            item.get("id").and_then(Value::as_str) == Some(item_id)
                && item.get("disabled") != Some(&json!(true))
        }) {
            return Err("menu action is no longer available".into());
        }
        return builtin_execute(&ctx, item_id).await;
    }
    let plugin_id = provider_id
        .strip_prefix(PLUGIN_PREFIX)
        .ok_or("unknown menu provider")?;
    let plugin = crate::plugins::host::catalog()
        .into_iter()
        .find(|plugin| plugin.id == plugin_id)
        .ok_or("menu provider unavailable")?;
    if !plugin.enabled
        || !plugin.approved
        || !plugin.backend_available
        || !matches!(
            plugin.extensions.get("connectionContextMenus"),
            Some(Value::Bool(true) | Value::Object(_))
        )
    {
        return Err("menu provider unavailable".into());
    }
    let session = menu_session_id
        .map(str::to_owned)
        .unwrap_or(crate::plugins::mounts::new_id()?);
    let offered = plugin_items(&plugin, &ctx, &session)?;
    if !offered.iter().any(|item| {
        item.get("kind").and_then(Value::as_str) == Some("action")
            && item.get("id").and_then(Value::as_str) == Some(item_id)
            && item.get("disabled") != Some(&json!(true))
    }) {
        return Err("menu action is no longer available".into());
    }
    let result = crate::plugins::host::global().invoke(
        &plugin.id,
        "connectionContextMenu.execute",
        json!({
            "menuSessionId": session,
            "interactionId": crate::plugins::mounts::new_id()?,
            "itemId": item_id,
            "connection": ctx.summary(),
        }),
    )?;
    crate::plugins::events::validate_result("connectionContextMenu.execute", &result)?;
    Ok(result)
}

fn plugin_items(
    plugin: &crate::plugins::host::PluginInfo,
    ctx: &Context,
    session: &str,
) -> Result<Vec<Value>, String> {
    crate::plugins::host::global()
        .invoke(
            &plugin.id,
            "connectionContextMenu.resolve",
            json!({
                "menuSessionId": session,
                "interactionId": crate::plugins::mounts::new_id()?,
                "connection": ctx.summary(),
            }),
        )
        .and_then(|value| valid_items(&value))
}

async fn builtin_execute(ctx: &Context, item_id: &str) -> Result<Value, String> {
    match item_id {
        "connection.edit" => Ok(json!({
            "kind": "coreAction",
            "command": "editConnection",
            "connectionId": ctx.info.id,
        })),
        "connection.close" => Ok(json!({
            "kind": "coreAction",
            "command": "closeConnection",
            "connectionId": ctx.info.id,
        })),
        "connection.saveSession" => {
            crate::remote::update_connection_session(&ctx.info.id, Some(true), None)?;
            Ok(json!({"kind": "handled"}))
        }
        "connection.toggleAutostart" => {
            crate::remote::update_connection_session(
                &ctx.info.id,
                None,
                Some(!ctx.info.autostart),
            )?;
            Ok(json!({"kind": "handled"}))
        }
        "connection.syncNow" => {
            crate::remote::reconcile_connection(&ctx.info.id).await;
            Ok(json!({"kind": "handled"}))
        }
        _ => Err("unknown connection menu action".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(plugin_id: &str, kind: &str, mount: &str, saved: bool, autostart: bool) -> Context {
        let plugin_view = mount.starts_with("fiex-plugin-view://");
        Context {
            info: ConnectionInfo {
                id: "c1".into(),
                name: "Connection".into(),
                kind: kind.into(),
                plugin_id: plugin_id.into(),
                mount_path: mount.into(),
                saved,
                autostart,
                can_read: true,
                can_write: false,
            },
            can_sync: !plugin_view && mount.starts_with("fiex://mount/"),
        }
    }

    fn ids(items: Vec<Value>) -> Vec<String> {
        items
            .into_iter()
            .filter_map(|item| item["id"].as_str().map(str::to_owned))
            .collect()
    }

    #[test]
    fn vfs_connections_offer_an_immediate_sync() {
        let menu = ctx(
            "org.fiex.openlist",
            "org.fiex.openlist",
            "fiex://mount/m/",
            true,
            false,
        );
        assert_eq!(
            ids(builtin_items("builtin:connection-sync", &menu).unwrap()),
            ["connection.syncNow"]
        );
        assert!(
            ids(builtin_items("builtin:connection-session", &menu).unwrap())
                .contains(&"connection.saveSession".into())
        );
    }

    #[test]
    fn terminal_and_plugin_views_do_not_offer_sync() {
        for menu in [
            ctx(
                "org.example.other",
                "other",
                "fiex-plugin-view://c1",
                true,
                false,
            ),
            ctx(
                "org.example.view",
                "ssh",
                "fiex-plugin-view://c1",
                true,
                false,
            ),
        ] {
            assert!(builtin_items("builtin:connection-sync", &menu)
                .unwrap()
                .is_empty());
        }
    }

    #[test]
    fn unsaved_sessions_can_be_saved_and_autostart_toggles() {
        let menu = ctx(
            "org.fiex.openlist",
            "org.fiex.openlist",
            "fiex://mount/m/",
            false,
            true,
        );
        let session = ids(builtin_items("builtin:connection-session", &menu).unwrap());
        assert_eq!(
            session,
            ["connection.saveSession", "connection.toggleAutostart"]
        );
        let close = builtin_items("builtin:connection-close", &menu).unwrap();
        assert_eq!(close[0]["placement"], "tail");
        assert_eq!(close[0]["danger"], true);
    }

    #[test]
    fn flat_plugin_items_are_validated_before_rendering() {
        let good = json!({"items":[{"id":"openlist.sync","kind":"action","title":"Sync"}]});
        assert_eq!(valid_items(&good).unwrap().len(), 1);
        for bad in [
            json!({"items":[{"id":"x","kind":"submenu","title":"X"}]}),
            json!({"items":[{"id":"x","kind":"action","title":"X","parentId":"p"}]}),
            json!({"items":[{"id":"x","kind":"action","title":"X"},{"id":"x","kind":"action","title":"Y"}]}),
        ] {
            assert!(valid_items(&bad).is_err(), "{bad}");
        }
    }
}
