//! Connection context menus: builtin providers plus plugin registrations.
//!
//! The connection is the menu target, not a file selection. Builtin providers
//! own the shell actions (edit, session, sync, close); plugins declaring
//! `extensions.connectionContextMenus` add their own flat action list.

use crate::plugins::{
    events::{EventRouter, ProviderCandidate, Resolver},
    native_invocation::NativeProviderInvocation,
};
mod native;
mod proxy;
mod validation;
use crate::remote::ConnectionInfo;
use fiex_provider_contracts::{
    ConnectionContextMenuProvider, MatchResource, MenuExecuteResult, MenuItem, ProviderError,
    ProviderInvocation, ProviderRequest, ResourceCapabilities, ResourceLocation, ResourceMetadata,
    ResourceType,
};
use native::providers;
use serde_json::{json, Value};
use std::sync::Arc;
use validation::valid_items;

#[cfg(test)]
use native::items as builtin_items;

#[derive(Clone)]
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
    fn matcher_resource(&self) -> MatchResource {
        let owner = format!("plugin:{}:{}", self.info.plugin_id, self.info.kind);
        let (_, storage) = crate::plugins::match_rules::typed_locations_for(&self.info.mount_path);
        MatchResource {
            metadata: Some(ResourceMetadata {
                kind: "connection".into(),
                ..Default::default()
            }),
            view: Some(ResourceLocation {
                resource_type: ResourceType::Vfs,
                provider_id: owner,
                mount_id: Some(self.info.id.clone()),
            }),
            storage,
            capabilities: ResourceCapabilities::default(),
        }
    }
}

fn implementation_for(
    resolver: &Resolver,
    context: &Context,
    session: &str,
) -> Arc<dyn ConnectionContextMenuProvider> {
    match resolver.plugin.as_deref() {
        None => native::provider(&resolver.id).unwrap_or_else(|| {
            Arc::new(FailedConnectionMenuProvider(
                "registered native connection-menu provider disappeared".into(),
            ))
        }),
        Some(plugin) => match proxy::from_plugin(plugin, context, session, &resolver.id) {
            Ok(proxy) => Arc::new(proxy),
            Err(error) => Arc::new(FailedConnectionMenuProvider(error)),
        },
    }
}

struct FailedConnectionMenuProvider(String);
impl ConnectionContextMenuProvider for FailedConnectionMenuProvider {
    fn resolve(&self, _ctx: &dyn ProviderInvocation) -> Result<Vec<MenuItem>, ProviderError> {
        Err(ProviderError::internal(self.0.clone()))
    }
    fn execute(&self, _ctx: &dyn ProviderInvocation) -> Result<MenuExecuteResult, ProviderError> {
        Err(ProviderError::internal(self.0.clone()))
    }
}

pub fn query(connection_id: &str) -> Result<Vec<Value>, String> {
    let ctx = Context::load(connection_id)?;
    let session = crate::plugins::mounts::new_id()?;
    let targets = [ctx.matcher_resource()];
    let providers = crate::plugins::provider_registry::candidates(
        "connectionContextMenu.resolve",
        &targets,
        &ctx.info.mount_path,
        providers(&targets),
    );
    let request = ProviderRequest {
        method: "connectionContextMenu.resolve".into(),
        params: json!({}),
    };
    let providers = providers
        .into_iter()
        .map(|resolver| ProviderCandidate {
            implementation: implementation_for(&resolver, &ctx, &session),
            resolver,
        })
        .collect();
    let invocation =
        NativeProviderInvocation::new(request.method.clone(), request.params.clone(), &[], None)
            .with_local_data(Arc::new(ctx.clone()));
    Ok(EventRouter::resolve_shared(
        "connectionContextMenu.resolve",
        providers,
        &invocation,
        |_, implementation, invocation| {
            implementation
                .resolve(invocation)
                .map(|items| Some(json!({"items":items})))
                .map_err(|error| error.to_string())
        },
        |provider, response| {
            let Some(response) = response else {
                return Ok(None);
            };
            crate::plugins::events::validate_result("connectionContextMenu.resolve", &response)?;
            let items = valid_items(&response)?;
            if items.is_empty() {
                return Ok(None);
            }
            let value = match provider.plugin.as_deref() {
                None => json!({
                    "kind":"builtin",
                    "providerId":provider.id,
                    "menuSessionId":session,
                    "items":items
                }),
                Some(plugin) => json!({
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

pub(crate) fn forget_plugin(plugin_id: &str) {
    proxy::forget_plugin(plugin_id);
}

/// A plugin result is never authority to run a privileged native command.
/// Only an already-matched Core builtin may return the private nativeAction
/// marker, and the action must be identical to the one Core revalidated.
fn native_action_authorized(
    builtin: bool,
    item_id: &str,
    result: &MenuExecuteResult,
) -> Result<bool, String> {
    match (builtin, result) {
        (true, MenuExecuteResult::NativeAction(action)) if action.item_id == item_id => Ok(true),
        (true, _) => Err("invalid native connection menu response".into()),
        (
            false,
            MenuExecuteResult::NativeAction(_)
            | MenuExecuteResult::CoreAction(_)
            | MenuExecuteResult::ConnectionCoreAction(_)
            | MenuExecuteResult::RemoteAction(_),
        ) => Err("plugin cannot execute native connection actions".into()),
        (false, _) => Ok(false),
    }
}

pub async fn execute(
    connection_id: &str,
    provider_id: &str,
    item_id: &str,
    menu_session_id: Option<&str>,
) -> Result<Value, String> {
    let ctx = Context::load(connection_id)?;
    let targets = [ctx.matcher_resource()];
    let owner = crate::plugins::provider_registry::candidates(
        "connectionContextMenu.execute",
        &targets,
        &ctx.info.mount_path,
        providers(&targets),
    )
    .into_iter()
    .find(|provider| provider.id == provider_id)
    .ok_or("menu provider is disabled or does not match connection")?;
    let builtin_owner = owner.plugin.is_none();
    let session = menu_session_id
        .map(str::to_owned)
        .unwrap_or(crate::plugins::mounts::new_id()?);
    let request = ProviderRequest {
        method: "connectionContextMenu.execute".into(),
        params: json!({"itemId":item_id}),
    };
    let implementation = implementation_for(&owner, &ctx, &session);
    let candidate = ProviderCandidate {
        resolver: owner,
        implementation,
    };
    let invocation =
        NativeProviderInvocation::new(request.method.clone(), request.params.clone(), &[], None)
            .with_local_data(Arc::new(ctx.clone()));
    let result = EventRouter::resolve_shared(
        "connectionContextMenu.execute",
        vec![candidate],
        &invocation,
        |_, implementation, invocation| {
            implementation
                .execute(invocation)
                .map_err(|error| error.to_string())
        },
        |_, result| {
            let wire = serde_json::to_value(&result).map_err(|error| error.to_string())?;
            crate::plugins::events::validate_result("connectionContextMenu.execute", &wire)?;
            Ok(result)
        },
        |_| true,
        |results| {
            results
                .into_iter()
                .next()
                .ok_or_else(|| "connection menu provider unavailable".to_string())?
                .1
        },
    )?;
    if native_action_authorized(builtin_owner, item_id, &result)? {
        return native::execute(provider_id, &ctx, item_id)
            .await
            .and_then(|result| serde_json::to_value(result).map_err(|error| error.to_string()));
    }
    serde_json::to_value(result).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests;
