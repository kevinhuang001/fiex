//! Connection context menus: builtin providers plus plugin registrations.
//!
//! The connection is the menu target, not a file selection. Builtin providers
//! own the shell actions (edit, session, sync, close); plugins declaring
//! `extensions.connectionContextMenus` add their own flat action list.

use crate::plugins::events::{EventRouter, Handler, Resolver};
mod endpoint;
mod validation;
use crate::remote::ConnectionInfo;
use fiex_provider_contracts::ProviderRequest;
use serde_json::{json, Value};
use validation::valid_items;

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

    /// Verified connection identity for the common ProviderMatchRule.
    /// The saved connection is authoritative; plugin JSON never supplies the
    /// view owner, storage owner, mount or effective operations.
    fn matcher_metadata(&self) -> Value {
        let owner = format!("plugin:{}:{}", self.info.plugin_id, self.info.kind);
        let (_, storage) = crate::plugins::match_rules::locations_for(&self.info.mount_path);
        json!({
            "kind":"connection",
            "view":{
                "type":"vfs",
                "providerId":owner,
                "mountId":self.info.id
            },
            "storage":storage,
            "operations":[]
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

pub fn query(connection_id: &str) -> Result<Vec<Value>, String> {
    let ctx = Context::load(connection_id)?;
    let session = crate::plugins::mounts::new_id()?;
    let metadata = [ctx.matcher_metadata()];
    let providers = crate::plugins::provider_registry::candidates(
        "connectionContextMenu.resolve",
        &metadata,
        &ctx.info.mount_path,
        providers(),
    );
    let request = ProviderRequest {
        method: "connectionContextMenu.resolve".into(),
        params: json!({}),
    };
    Ok(EventRouter::resolve(
        "connectionContextMenu.resolve",
        providers,
        &request,
        |provider| Ok(endpoint::from_resolver(provider, &ctx, &session)),
        |provider, response| {
            let items = valid_items(&response)?;
            if items.is_empty() {
                return Ok(None);
            }
            let value = match &provider.handler {
                Handler::Builtin(_) => json!({
                    "kind":"builtin",
                    "providerId":provider.id,
                    "menuSessionId":session,
                    "items":items
                }),
                Handler::Plugin(plugin) => json!({
                    "kind":"plugin",
                    "providerId":provider.id,
                    "pluginId":plugin.id,
                    "pluginName":plugin.name,
                    "menuSessionId":session,
                    "items":items
                }),
            };
            Ok(Some(value))
        },
        |_| true,
        |results| {
            results
                .into_iter()
                .filter_map(|(_, result)| result.ok().flatten())
                .collect()
        },
    ))
}

pub async fn execute(
    connection_id: &str,
    provider_id: &str,
    item_id: &str,
    menu_session_id: Option<&str>,
) -> Result<Value, String> {
    let ctx = Context::load(connection_id)?;
    let metadata = [ctx.matcher_metadata()];
    let owner = crate::plugins::provider_registry::candidates(
        "connectionContextMenu.execute",
        &metadata,
        &ctx.info.mount_path,
        providers(),
    )
    .into_iter()
    .find(|provider| provider.id == provider_id)
    .ok_or("menu provider is disabled or does not match connection")?;
    let session = menu_session_id
        .map(str::to_owned)
        .unwrap_or(crate::plugins::mounts::new_id()?);
    let request = ProviderRequest {
        method: "connectionContextMenu.execute".into(),
        params: json!({"itemId":item_id}),
    };
    let result = EventRouter::resolve(
        "connectionContextMenu.execute",
        vec![owner],
        &request,
        |provider| Ok(endpoint::from_resolver(provider, &ctx, &session)),
        |_, result| Ok(result),
        |_| true,
        |results| {
            results
                .into_iter()
                .next()
                .ok_or_else(|| "connection menu provider unavailable".to_string())?
                .1
        },
    )?;
    if result["kind"] == "nativeAction" {
        if result["itemId"] != item_id {
            return Err("invalid native connection action".into());
        }
        return builtin_execute(&ctx, item_id).await;
    }
    Ok(result)
}

fn plugin_items(
    plugin: &crate::plugins::host::PluginInfo,
    ctx: &Context,
    session: &str,
    provider_id: Option<&str>,
) -> Result<Vec<Value>, String> {
    let mut params = json!({
        "menuSessionId": session,
        "interactionId": crate::plugins::mounts::new_id()?,
        "connection": ctx.summary(),
    });
    if let Some(id) = provider_id {
        params["providerId"] = json!(id);
    }
    crate::plugins::host::global()
        .invoke(&plugin.id, "connectionContextMenu.resolve", params)
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
mod tests;
