//! Shared ConnectionContextMenuProvider implementations for native and
//! plugin owners. Core keeps connection identity and rechecks every action.
use super::{builtin_items, plugin_items, Context};
use crate::plugins::host::PluginInfo;
use fiex_provider_contracts::{ConnectionContextMenuProvider, ProviderError, ProviderInvocation};
use serde_json::{json, Value};

pub(super) struct NativeMenu<'a> {
    pub id: String,
    pub ctx: &'a Context,
}

impl ConnectionContextMenuProvider for NativeMenu<'_> {
    fn resolve(&self, _ctx: &dyn ProviderInvocation) -> Result<Vec<Value>, ProviderError> {
        builtin_items(&self.id, self.ctx).map_err(ProviderError::internal)
    }

    fn execute(&self, ctx: &dyn ProviderInvocation) -> Result<Value, ProviderError> {
        let item = ctx.params()["itemId"]
            .as_str()
            .ok_or_else(|| ProviderError::invalid_params("missing menu action"))?;
        let items = builtin_items(&self.id, self.ctx).map_err(ProviderError::internal)?;
        if !items.iter().any(|value| {
            value["id"] == item && value["disabled"] != true && value["kind"] == "action"
        }) {
            return Err(ProviderError::invalid_params(
                "menu action is no longer available",
            ));
        }
        // Async Core operations run after this native-only action is validated.
        Ok(json!({"kind":"nativeAction","itemId":item}))
    }
}

pub(super) struct PluginMenu<'a> {
    pub plugin: PluginInfo,
    pub ctx: &'a Context,
    pub session: &'a str,
    pub provider_id: Option<String>,
}

impl PluginMenu<'_> {
    fn params(&self, item: Option<&str>) -> Result<Value, String> {
        let mut params = json!({
            "menuSessionId":self.session,
            "interactionId":crate::plugins::mounts::new_id()?,
            "connection":self.ctx.summary(),
        });
        if let Some(id) = &self.provider_id {
            params["providerId"] = json!(id);
        }
        if let Some(item) = item {
            params["itemId"] = json!(item);
        }
        Ok(params)
    }
}

impl ConnectionContextMenuProvider for PluginMenu<'_> {
    fn resolve(&self, _ctx: &dyn ProviderInvocation) -> Result<Vec<Value>, ProviderError> {
        plugin_items(
            &self.plugin,
            self.ctx,
            self.session,
            self.provider_id.as_deref(),
        )
        .map_err(ProviderError::internal)
    }

    fn execute(&self, ctx: &dyn ProviderInvocation) -> Result<Value, ProviderError> {
        let item = ctx.params()["itemId"]
            .as_str()
            .ok_or_else(|| ProviderError::invalid_params("missing menu action"))?;
        let offered = plugin_items(
            &self.plugin,
            self.ctx,
            self.session,
            self.provider_id.as_deref(),
        )
        .map_err(ProviderError::internal)?;
        if !offered.iter().any(|value| {
            value["kind"] == "action" && value["id"] == item && value["disabled"] != true
        }) {
            return Err(ProviderError::invalid_params(
                "menu action is no longer available",
            ));
        }
        crate::plugins::host::global()
            .invoke(
                &self.plugin.id,
                "connectionContextMenu.execute",
                self.params(Some(item)).map_err(ProviderError::internal)?,
            )
            .map_err(ProviderError::internal)
    }
}

pub(super) fn from_resolver<'a>(
    resolver: &crate::plugins::events::Resolver,
    ctx: &'a Context,
    session: &'a str,
) -> Box<dyn ConnectionContextMenuProvider + 'a> {
    match &resolver.handler {
        crate::plugins::events::Handler::Builtin(id) => Box::new(NativeMenu {
            id: (*id).into(),
            ctx,
        }),
        crate::plugins::events::Handler::Plugin(plugin) => Box::new(PluginMenu {
            plugin: (**plugin).clone(),
            ctx,
            session,
            provider_id: resolver
                .id
                .strip_prefix(&format!("plugin:{}:", plugin.id))
                .map(str::to_owned),
        }),
    }
}
