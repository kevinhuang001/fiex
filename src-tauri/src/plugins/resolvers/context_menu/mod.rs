//! Shared EventRouter pipeline for builtin and plugin menus.
mod actions;
mod builtin;
mod connection_cache;
mod endpoint;
mod file_ops;
mod native_views;
mod proxy;
pub use actions::execute_remote;
#[cfg(test)]
mod tests;
mod validation;

pub(crate) fn forget_plugin(plugin_id: &str) {
    proxy::forget_plugin(plugin_id);
    if let Ok(mut sessions) = sessions().lock() {
        sessions.retain(|_, session| {
            !matches!(&session.provider.handler, Handler::Plugin(plugin) if plugin.id == plugin_id)
        });
    }
}

use crate::plugins::events::{EventRouter, Handler, Resolver};
use crate::plugins::{
    native_invocation::NativeProviderInvocation, resource::resource_context::ResourceSnapshot,
};
use fiex_provider_contracts::{ContextMenuProvider, ProviderRequest};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, OnceLock},
    time::Instant,
};
use validation::{root_menu_valid, submenu_depths, validated_submenu};

struct Session {
    provider: Resolver,
    /// Remote SDK session is private to its owning plugin; never use it as the
    /// Core/UI session key or allow it to overwrite another provider's state.
    remote_session_id: Option<String>,
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
/// Generate an independent Core session ID for every menu, regardless of
/// which provider created it. Only IPC providers retain their transport token.
fn session_ids(result: &Value, is_plugin: bool) -> Result<(String, Option<String>), String> {
    let remote = if is_plugin {
        let id = result["menuSessionId"]
            .as_str()
            .filter(|id| !id.is_empty() && id.len() <= 128)
            .ok_or("plugin menu session ID is missing or invalid")?;
        Some(id.to_owned())
    } else {
        None
    };
    Ok((crate::plugins::mounts::new_id()?, remote))
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
    let (session_id, remote_session_id) =
        session_ids(&result, matches!(&provider.handler, Handler::Plugin(_)))?;
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
            remote_session_id,
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

fn invoke_provider(
    provider: &Resolver,
    request: &ProviderRequest,
    context: Option<&builtin::Context>,
    resources: &Arc<Vec<ResourceSnapshot>>,
    directory: &str,
) -> Result<Value, String> {
    let request = crate::plugins::events::bind_provider_request(provider, request)?;
    match &provider.handler {
        Handler::Builtin(id) => {
            let context = context.ok_or("native menu context is unavailable")?;
            let endpoint = endpoint::NativeMenuEndpoint {
                id: (*id).into(),
                context,
            };
            let invocation = NativeProviderInvocation::new(
                request.method.clone(),
                request.params.clone(),
                resources,
                None,
            );
            match request.method.as_str() {
                "contextMenu.resolve" => {
                    let items = ContextMenuProvider::resolve(&endpoint, &invocation)
                        .map_err(|error| error.to_string())?;
                    Ok(json!({"items":items}))
                }
                "contextMenu.execute" => ContextMenuProvider::execute(&endpoint, &invocation)
                    .map_err(|error| error.to_string()),
                _ => Err("unsupported ContextMenu event".into()),
            }
        }
        Handler::Plugin(plugin) => {
            let proxy = proxy::ScopedMenuProxy::new(
                (**plugin).clone(),
                directory.into(),
                resources.clone(),
            )?;
            match request.method.as_str() {
                "contextMenu.resolve" => proxy.resolve_typed(&request),
                "contextMenu.execute" => proxy.execute_typed(&request),
                _ => Err("unsupported ContextMenu event".into()),
            }
        }
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
    let match_targets = crate::plugins::events::matching::match_resources(&resources);
    let mut providers = crate::plugins::provider_registry::candidates(
        "contextMenu.resolve",
        &match_targets,
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
    Ok(EventRouter::resolve_typed(
        "contextMenu.resolve",
        providers,
        &request,
        |resolver, request| invoke_provider(resolver, request, Some(&ctx), &resources, directory),
        |provider, result| {
            crate::plugins::events::validate_result("contextMenu.resolve", &result)?;
            register(provider, &ctx, &resources, result)
        },
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
    let (provider, remote_session_id, directory, resources, depth) = {
        let registry = sessions().lock().map_err(|_| "menu registry unavailable")?;
        let session = registry.get(session_id).ok_or("menu session expired")?;
        if session.created.elapsed().as_secs() >= 90 || !allowed(session) {
            return Err("menu session expired".into());
        }
        if !matches!(&session.provider.handler, Handler::Plugin(_)) {
            return Err("builtin submenu is not lazy".into());
        }
        (
            session.provider.clone(),
            session
                .remote_session_id
                .clone()
                .ok_or("plugin session missing")?,
            session.directory.clone(),
            session.resources.clone(),
            *session.submenus.get(parent_id).ok_or("unknown submenu")?,
        )
    };
    let params = json!({"menuSessionId":remote_session_id,"parentId":parent_id,
        "interactionId":crate::plugins::mounts::new_id()?});
    let request = ProviderRequest {
        method: "contextMenu.resolve".into(),
        params,
    };
    let result = EventRouter::resolve_typed(
        "contextMenu.resolve",
        vec![provider.clone()],
        &request,
        |resolver, request| invoke_provider(resolver, request, None, &resources, &directory),
        |_, result| {
            crate::plugins::events::validate_result("contextMenu.resolve", &result)?;
            Ok(result)
        },
        |_| true,
        |results| {
            results
                .into_iter()
                .next()
                .ok_or_else(|| "menu provider unavailable".to_string())?
                .1
        },
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
            "menuSessionId": session.remote_session_id.as_deref().unwrap_or(session_id),
            "itemId": item_id,
            "interactionId": crate::plugins::mounts::new_id()?,
        }),
    };
    EventRouter::resolve_typed(
        "contextMenu.execute",
        vec![session.provider.clone()],
        &request,
        |resolver, request| {
            invoke_provider(
                resolver,
                request,
                Some(&ctx),
                &session.resources,
                &session.directory,
            )
        },
        |_, result| {
            crate::plugins::events::validate_result("contextMenu.execute", &result)?;
            Ok(result)
        },
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

#[cfg(test)]
mod session_identity_tests {
    use super::*;

    #[test]
    fn plugin_session_tokens_never_become_core_menu_handles() {
        let reply = json!({"menuSessionId":"the-same-remote-session"});
        let (first, remote_first) = session_ids(&reply, true).unwrap();
        let (second, remote_second) = session_ids(&reply, true).unwrap();
        assert_ne!(first, second);
        assert_ne!(first, "the-same-remote-session");
        assert_eq!(remote_first.as_deref(), Some("the-same-remote-session"));
        assert_eq!(remote_second.as_deref(), Some("the-same-remote-session"));
        let (_, native) = session_ids(&reply, false).unwrap();
        assert!(native.is_none());
        assert!(session_ids(&json!({}), true).is_err());
        assert!(session_ids(&json!({"menuSessionId":""}), true).is_err());
        assert!(session_ids(&json!({"menuSessionId":"x".repeat(129)}), true).is_err());
    }
}
