//! Persistent ConnectionContextMenu IPC identities with per-invocation
//! connection data and menu-session scope.
use super::Context;
use crate::plugins::host::PluginInfo;
use fiex_provider_contracts::{
    ConnectionContextMenuProvider, MenuExecuteResult, MenuItem, ProviderDescriptor, ProviderError,
    ProviderInput, ProviderInstances, ProviderInvocation, ProviderKind, ProviderMatchRule,
    ProviderRequestContext,
};
use serde_json::{json, Value};
use std::{
    any::Any,
    sync::{Arc, Mutex, OnceLock},
};

struct ConnectionMenuServices {
    plugin: PluginInfo,
    context: Context,
    session: String,
    provider_id: Option<String>,
}

struct CoreConnectionMenuInvocation {
    request: ProviderRequestContext,
    services: ConnectionMenuServices,
}

impl CoreConnectionMenuInvocation {
    fn new(
        request: ProviderRequestContext,
        plugin: PluginInfo,
        context: Context,
        session: String,
        provider_id: Option<String>,
    ) -> Self {
        Self {
            request,
            services: ConnectionMenuServices {
                plugin,
                context,
                session,
                provider_id,
            },
        }
    }
}

impl ProviderInvocation for CoreConnectionMenuInvocation {
    fn request(&self) -> &ProviderRequestContext {
        &self.request
    }

    fn local_service_handle(&self) -> Option<&dyn Any> {
        Some(&self.services)
    }
}

struct IpcConnectionMenu {
    plugin_id: String,
}

impl IpcConnectionMenu {
    fn services<'a>(
        &self,
        ctx: &'a dyn ProviderInvocation,
        method: &str,
    ) -> Result<&'a ConnectionMenuServices, ProviderError> {
        let services = ctx
            .local_service_handle()
            .and_then(|value| value.downcast_ref::<ConnectionMenuServices>())
            .ok_or_else(|| ProviderError::invalid_params("missing connection menu scope"))?;
        if services.plugin.id != self.plugin_id || ctx.method() != method {
            return Err(ProviderError::invalid_params(
                "ConnectionContextMenuProvider identity or event mismatch",
            ));
        }
        Ok(services)
    }

    fn params(
        services: &ConnectionMenuServices,
        item: Option<&str>,
    ) -> Result<Value, ProviderError> {
        let mut params = json!({
            "menuSessionId": services.session,
            "interactionId": crate::plugins::mounts::new_id().map_err(ProviderError::internal)?,
            "connection": services.context.summary(),
        });
        if let Some(id) = &services.provider_id {
            params["providerId"] = json!(id);
        }
        if let Some(item) = item {
            params["itemId"] = json!(item);
        }
        Ok(params)
    }

    fn plugin_items(
        &self,
        services: &ConnectionMenuServices,
    ) -> Result<Vec<MenuItem>, ProviderError> {
        let result = crate::plugins::events::invoke(
            &services.plugin.id,
            "connectionContextMenu.resolve",
            Self::params(services, None)?,
        )
        .map_err(ProviderError::internal)?;
        let items = crate::plugins::providers::connection_menu::validation::valid_items(&result)
            .map_err(ProviderError::invalid_params)?;
        serde_json::from_value(serde_json::Value::Array(items))
            .map_err(|error| ProviderError::invalid_params(error.to_string()))
    }
}

impl ConnectionContextMenuProvider for IpcConnectionMenu {
    fn resolve(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<MenuItem>, ProviderError> {
        self.plugin_items(self.services(ctx, "connectionContextMenu.resolve")?)
    }

    fn execute(&self, ctx: &dyn ProviderInvocation) -> Result<MenuExecuteResult, ProviderError> {
        let services = self.services(ctx, "connectionContextMenu.execute")?;
        let item = match ctx.input()? {
            ProviderInput::ConnectionMenuExecute(request) => request.item_id.as_deref(),
            _ => None,
        }
        .ok_or_else(|| ProviderError::invalid_params("missing menu action"))?;
        let offered = self.plugin_items(services)?;
        if !offered.iter().any(|value| {
            value.kind == fiex_provider_contracts::MenuItemKind::Action
                && value.id == item
                && !value.disabled
        }) {
            return Err(ProviderError::invalid_params(
                "menu action is no longer available",
            ));
        }
        let result = crate::plugins::events::invoke(
            &services.plugin.id,
            "connectionContextMenu.execute",
            Self::params(services, Some(item))?,
        )
        .map_err(ProviderError::internal)?;
        serde_json::from_value(result)
            .map_err(|error| ProviderError::invalid_params(error.to_string()))
    }
}

fn registry() -> &'static Mutex<ProviderInstances<dyn ConnectionContextMenuProvider>> {
    static REGISTERED: OnceLock<Mutex<ProviderInstances<dyn ConnectionContextMenuProvider>>> =
        OnceLock::new();
    REGISTERED
        .get_or_init(|| Mutex::new(ProviderInstances::new(ProviderKind::ConnectionContextMenu)))
}

pub(super) fn forget_plugin(plugin_id: &str) {
    if let Ok(mut providers) = registry().lock() {
        providers.unregister(&format!("plugin:{plugin_id}"));
    }
}

fn proxy(plugin_id: &str) -> Result<Arc<dyn ConnectionContextMenuProvider>, String> {
    if plugin_id.is_empty() {
        return Err("missing ConnectionContextMenuProvider plugin identity".into());
    }
    let mut providers = registry()
        .lock()
        .map_err(|_| "ConnectionContextMenuProvider registry unavailable".to_string())?;
    let id = format!("plugin:{plugin_id}");
    if let Some(provider) = providers.get_arc(&id) {
        return Ok(provider);
    }
    providers.register(
        ProviderDescriptor {
            id: id.clone(),
            kind: ProviderKind::ConnectionContextMenu,
            priority: 50,
            r#match: ProviderMatchRule::default(),
        },
        Arc::new(IpcConnectionMenu {
            plugin_id: plugin_id.into(),
        }),
    )?;
    providers
        .get_arc(&id)
        .ok_or_else(|| "ConnectionContextMenuProvider registration failed".into())
}

pub(super) struct ScopedConnectionMenuProxy {
    handler: Arc<dyn ConnectionContextMenuProvider>,
    plugin: PluginInfo,
    context: Context,
    session: String,
    provider_id: Option<String>,
    registered_id: String,
}

impl ScopedConnectionMenuProxy {
    fn invocation(
        &self,
        ctx: &dyn ProviderInvocation,
        method: &str,
    ) -> Result<CoreConnectionMenuInvocation, ProviderError> {
        if ctx.method() != method {
            return Err(ProviderError::invalid_params(
                "ConnectionContextMenuProvider method mismatch",
            ));
        }
        let request = crate::plugins::events::bind_provider_context(
            &self.plugin.id,
            &self.registered_id,
            ctx.request(),
        )
        .map_err(ProviderError::invalid_params)?;
        Ok(CoreConnectionMenuInvocation::new(
            request,
            self.plugin.clone(),
            self.context.clone(),
            self.session.clone(),
            self.provider_id.clone(),
        ))
    }
}

impl ConnectionContextMenuProvider for ScopedConnectionMenuProxy {
    fn resolve(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<MenuItem>, ProviderError> {
        let invocation = self.invocation(ctx, "connectionContextMenu.resolve")?;
        self.handler.resolve(&invocation)
    }

    fn execute(&self, ctx: &dyn ProviderInvocation) -> Result<MenuExecuteResult, ProviderError> {
        let invocation = self.invocation(ctx, "connectionContextMenu.execute")?;
        self.handler.execute(&invocation)
    }
}

pub(super) fn from_plugin(
    plugin: &PluginInfo,
    context: &Context,
    session: &str,
    resolver_id: &str,
) -> Result<ScopedConnectionMenuProxy, String> {
    Ok(ScopedConnectionMenuProxy {
        handler: proxy(&plugin.id)?,
        plugin: plugin.clone(),
        context: context.clone(),
        session: session.into(),
        provider_id: resolver_id
            .strip_prefix(&format!("plugin:{}:", plugin.id))
            .map(str::to_owned),
        registered_id: resolver_id.into(),
    })
}
