//! Shared EventRouter pipeline for builtin and plugin menus.
mod actions;
mod builtin;
mod connection_cache;
mod endpoint;
mod file_ops;
mod native_views;
pub use actions::execute_remote;
#[cfg(test)]
mod tests;
mod validation;

use crate::plugins::events::{EventRouter, Handler, Resolver};
use crate::plugins::resource::resource_context::ResourceSnapshot;
use fiex_provider_contracts::{ProviderEndpoint, ProviderRequest};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, OnceLock},
    time::Instant,
};
use validation::{root_menu_valid, submenu_depths, validated_submenu};

struct Session {
    provider: Resolver,
    directory: String,
    paths: Vec<String>,
    resources: Arc<Vec<ResourceSnapshot>>,
    actions: HashSet<String>,
    submenus: HashMap<String, usize>,
    created: Instant,
}
/// Menu resolution is split by owner so the top-level Core menu never waits for
/// plugin IPC. `All` is kept for callers that explicitly want one batch.
#[derive(Clone, Copy, PartialEq, Eq)]
enum MenuScope {
    All,
    Core,
    Plugins,
}
fn sessions() -> &'static Mutex<HashMap<String, Session>> {
    static SESSIONS: OnceLock<Mutex<HashMap<String, Session>>> = OnceLock::new();
    SESSIONS.get_or_init(Default::default)
}
fn allowed(session: &Session) -> bool {
    match &session.provider.handler {
        Handler::Builtin(_) => true,
        Handler::Plugin(plugin) => crate::plugins::events::menu_allowed(
            &plugin.id,
            &session.provider.id,
            &session.resources,
            &session.directory,
        ),
    }
}
fn register(
    provider: &Resolver,
    ctx: &builtin::Context,
    resources: &Arc<Vec<ResourceSnapshot>>,
    result: Value,
) -> Result<Value, String> {
    let actions = root_menu_valid(&result)?;
    let items = result["items"].as_array().ok_or("invalid menu items")?;
    if items.is_empty() {
        return Ok(json!({"items":[]}));
    }
    let session_id = result["menuSessionId"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or(crate::plugins::mounts::new_id()?);
    let mut registry = sessions().lock().map_err(|_| "menu registry unavailable")?;
    registry.retain(|_, session| session.created.elapsed().as_secs() < 90);
    // Bound memory while allowing fast menu navigation with multiple builtin providers.
    if registry.len() >= 512 {
        if let Some(oldest) = registry
            .iter()
            .min_by_key(|(_, session)| session.created)
            .map(|(id, _)| id.clone())
        {
            registry.remove(&oldest);
        }
    }
    registry.insert(
        session_id.clone(),
        Session {
            provider: provider.clone(),
            directory: ctx.directory.clone(),
            paths: ctx.paths.clone(),
            resources: resources.clone(),
            actions,
            submenus: submenu_depths(&result),
            created: Instant::now(),
        },
    );
    match &provider.handler {
        Handler::Builtin(_) => Ok(
            json!({"kind":"builtin","providerId":provider.id,"menuSessionId":session_id,"items":items}),
        ),
        Handler::Plugin(plugin) => Ok(
            json!({"kind":"plugin","providerId":provider.id,"pluginId":plugin.id,"menuSessionId":session_id,"items":items}),
        ),
    }
}
pub fn query(directory: &str, paths: &[String]) -> Result<Vec<Value>, String> {
    query_scope(directory, paths, MenuScope::All)
}

/// Top-level, Core-owned items. This path must stay free of plugin IPC so the
/// first visible menu is complete and cannot resize while plugins respond.
pub fn query_core(directory: &str, paths: &[String]) -> Result<Vec<Value>, String> {
    query_scope(directory, paths, MenuScope::Core)
}

/// Plugin-owned items, requested by the lazy "插件" submenu.
pub fn query_plugins(directory: &str, paths: &[String]) -> Result<Vec<Value>, String> {
    query_scope(directory, paths, MenuScope::Plugins)
}

fn query_scope(directory: &str, paths: &[String], scope: MenuScope) -> Result<Vec<Value>, String> {
    // An empty selection is the blank canvas. It uses the same provider
    // pipeline; manifests opt in with `match.selection: "zero"` (or `any`).
    if paths.len() > 256 || directory.is_empty() {
        return Err("invalid menu selection".into());
    }
    let ctx = builtin::Context::load(directory, paths)?;
    let resources = Arc::new(
        ctx.entries
            .iter()
            .map(|entry| ResourceSnapshot::from_entry(&entry.path, entry))
            .collect::<Vec<_>>(),
    );
    let metadata = crate::plugins::events::resource_metadata(&resources);
    let mut providers = crate::plugins::provider_registry::candidates(
        "contextMenu.resolve",
        &metadata,
        directory,
        if scope == MenuScope::Plugins {
            Vec::new()
        } else {
            builtin::providers(&ctx)
        },
    );
    // Never invoke a plugin while resolving the Core-only top-level menu. For a
    // plugin-only request the empty seed above already excludes builtin rows.
    if scope == MenuScope::Core {
        providers.retain(|provider| matches!(&provider.handler, Handler::Builtin(_)));
    }
    let request = ProviderRequest {
        method: "contextMenu.resolve".into(),
        params: json!({}),
    };
    Ok(EventRouter::resolve(
        "contextMenu.resolve",
        providers,
        &request,
        |resolver| {
            let endpoint: Box<dyn ProviderEndpoint + '_> = match &resolver.handler {
                Handler::Builtin(id) => Box::new(endpoint::NativeMenuEndpoint {
                    id: (*id).into(),
                    context: &ctx,
                }),
                Handler::Plugin(plugin) => Box::new(endpoint::PluginMenuProxy {
                    plugin: (**plugin).clone(),
                    directory: directory.into(),
                    resources: resources.clone(),
                }),
            };
            Ok(endpoint)
        },
        |provider, result| register(provider, &ctx, &resources, result),
        |_| true,
        |results| {
            results
                .into_iter()
                .filter_map(|(_, result)| {
                    result.ok().filter(|value| {
                        value["items"]
                            .as_array()
                            .is_some_and(|items| !items.is_empty())
                    })
                })
                .collect()
        },
    ))
}

pub fn children(session_id: &str, parent_id: &str) -> Result<Value, String> {
    let (plugin_id, instance_id, directory, resources, depth) = {
        let registry = sessions().lock().map_err(|_| "menu registry unavailable")?;
        let session = registry.get(session_id).ok_or("menu session expired")?;
        if session.created.elapsed().as_secs() >= 90 || !allowed(session) {
            return Err("menu session expired".into());
        }
        let Handler::Plugin(plugin) = &session.provider.handler else {
            return Err("builtin submenu is not lazy".into());
        };
        (
            plugin.id.clone(),
            session
                .provider
                .id
                .strip_prefix(&format!("plugin:{}:", plugin.id))
                .map(str::to_owned),
            session.directory.clone(),
            session.resources.clone(),
            *session.submenus.get(parent_id).ok_or("unknown submenu")?,
        )
    };
    let mut params = json!({"menuSessionId":session_id,"parentId":parent_id,
        "interactionId":crate::plugins::mounts::new_id()?});
    if let Some(id) = instance_id {
        params["providerId"] = json!(id);
    }
    let result = crate::plugins::events::call_owner(
        "contextMenu.resolve",
        &plugin_id,
        params,
        &resources,
        Some(&directory),
    )?;
    let validated = validated_submenu(&result, parent_id, depth)?;
    let mut registry = sessions().lock().map_err(|_| "menu registry unavailable")?;
    let session = registry.get_mut(session_id).ok_or("menu session expired")?;
    if session.created.elapsed().as_secs() >= 90 || !allowed(session) {
        return Err("menu session expired".into());
    }
    if validated
        .actions
        .iter()
        .any(|id| session.actions.contains(id) || session.submenus.contains_key(id))
        || validated
            .submenus
            .keys()
            .any(|id| session.actions.contains(id) || session.submenus.contains_key(id))
    {
        return Err("duplicate menu item id".into());
    }
    if session.actions.len()
        + session.submenus.len()
        + validated.actions.len()
        + validated.submenus.len()
        > 512
    {
        return Err("menu session item limit exceeded".into());
    }
    session.actions.extend(validated.actions);
    session.submenus.extend(validated.submenus);
    Ok(json!({"items":validated.items}))
}
pub fn execute(session_id: &str, item_id: &str) -> Result<Value, String> {
    let session = sessions()
        .lock()
        .map_err(|_| "menu registry unavailable")?
        .remove(session_id)
        .ok_or("menu session invalid or already used")?;
    if session.created.elapsed().as_secs() >= 90
        || !session.actions.contains(item_id)
        || !allowed(&session)
    {
        return Err("menu session expired or item was not offered".into());
    }
    let ctx = builtin::Context::load(&session.directory, &session.paths)?;
    let request = ProviderRequest {
        method: "contextMenu.execute".into(),
        params: json!({
            "menuSessionId": session_id,
            "itemId": item_id,
            "interactionId": crate::plugins::mounts::new_id()?,
        }),
    };
    EventRouter::resolve(
        "contextMenu.execute",
        vec![session.provider.clone()],
        &request,
        |resolver| {
            let endpoint: Box<dyn ProviderEndpoint + '_> = match &resolver.handler {
                Handler::Builtin(id) => Box::new(endpoint::NativeMenuEndpoint {
                    id: (*id).into(),
                    context: &ctx,
                }),
                Handler::Plugin(plugin) => Box::new(endpoint::PluginMenuProxy {
                    plugin: (**plugin).clone(),
                    directory: session.directory.clone(),
                    resources: session.resources.clone(),
                }),
            };
            Ok(endpoint)
        },
        |_, result| Ok(result),
        |_| true,
        |results| {
            results
                .into_iter()
                .next()
                .ok_or_else(|| "menu owner unavailable".to_string())?
                .1
        },
    )
}
