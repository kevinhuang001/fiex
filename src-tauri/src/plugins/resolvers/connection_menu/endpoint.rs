//! The same connection-menu endpoint contract is used by native menus and
//! plugin IPC. The Core owns connection identity and the permission boundary.
use super::{builtin_items, plugin_items, Context};
use crate::plugin_sdk::provider_contracts::{ProviderEndpoint, ProviderRequest};
use crate::plugins::host::PluginInfo;
use serde_json::{json, Value};

pub(super) struct NativeMenu<'a> {
    pub id: String,
    pub ctx: &'a Context,
}

impl ProviderEndpoint for NativeMenu<'_> {
    fn invoke(&self, request: &ProviderRequest) -> Result<Value, String> {
        match request.method.as_str() {
            "connectionContextMenu.resolve" => {
                Ok(json!({"items":builtin_items(&self.id, self.ctx)?}))
            }
            "connectionContextMenu.execute" => {
                let item = request.params["itemId"].as_str().ok_or("missing menu action")?;
                let items = builtin_items(&self.id, self.ctx)?;
                if !items.iter().any(|value| {
                    value["id"] == item && value["disabled"] != true
                        && value["kind"] == "action"
                }) {
                    return Err("menu action is no longer available".into());
                }
                // Async Core operations run *after* the shared endpoint
                // router validates this native-only, unexposed action.
                Ok(json!({"kind":"nativeAction","itemId":item}))
            }
            _ => Err("invalid connection-menu event".into()),
        }
    }
}

pub(super) struct PluginMenu<'a> {
    pub plugin: PluginInfo,
    pub ctx: &'a Context,
    pub session: &'a str,
}

impl PluginMenu<'_> {
    fn params(&self, item: Option<&str>) -> Result<Value, String> {
        let mut params = json!({
            "menuSessionId":self.session,
            "interactionId":crate::plugins::mounts::new_id()?,
            "connection":self.ctx.summary(),
        });
        if let Some(item) = item {
            params["itemId"] = json!(item);
        }
        Ok(params)
    }
}

impl ProviderEndpoint for PluginMenu<'_> {
    fn invoke(&self, request: &ProviderRequest) -> Result<Value, String> {
        match request.method.as_str() {
            "connectionContextMenu.resolve" => {
                Ok(json!({"items":plugin_items(&self.plugin, self.ctx, self.session)?}))
            }
            "connectionContextMenu.execute" => {
                let item = request.params["itemId"].as_str().ok_or("missing menu action")?;
                let offered = plugin_items(&self.plugin, self.ctx, self.session)?;
                if !offered.iter().any(|value| {
                    value["kind"] == "action"
                        && value["id"] == item
                        && value["disabled"] != true
                }) {
                    return Err("menu action is no longer available".into());
                }
                crate::plugins::host::global().invoke(
                    &self.plugin.id,
                    "connectionContextMenu.execute",
                    self.params(Some(item))?,
                )
            }
            _ => Err("invalid connection-menu event".into()),
        }
    }
}

pub(super) fn from_resolver<'a>(
    resolver: &crate::plugins::events::Resolver,
    ctx: &'a Context,
    session: &'a str,
) -> Box<dyn ProviderEndpoint + 'a> {
    match &resolver.handler {
        crate::plugins::events::Handler::Builtin(id) => Box::new(NativeMenu {
            id: (*id).into(),
            ctx,
        }),
        crate::plugins::events::Handler::Plugin(plugin) => Box::new(PluginMenu {
            plugin: (**plugin).clone(),
            ctx,
            session,
        }),
    }
}
