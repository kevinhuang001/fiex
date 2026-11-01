//! Shared EventRouter pipeline for builtin and plugin menus.
mod actions;
mod builtin;
mod connection_cache;
mod file_ops;
mod native_views;
mod proxy;
mod session;
pub use actions::execute_remote;
#[cfg(test)]
mod tests;
mod validation;

pub(crate) fn forget_plugin(plugin_id: &str) {
    proxy::forget_plugin(plugin_id);
    if let Ok(mut sessions) = sessions().lock() {
        sessions.retain(|_, session| {
            session.provider.plugin.as_deref().is_none_or(|plugin| plugin.id != plugin_id)
        });
    }
}

use crate::plugins::events::{EventRouter, ProviderCandidate, Resolver};
use crate::plugins::{
    native_invocation::NativeProviderInvocation, resource::resource_context::ResourceSnapshot,
    provider_registry::native::NativeProviderRegistry,
};
use fiex_provider_contracts::{
    ContextMenuProvider, MenuExecuteResult, MenuItem, ProviderKind, ProviderRequest,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};
use session::{allowed, register, sessions, session_ids, MenuScope, Session};
use validation::validated_submenu;

/// All Core-owned context-menu providers share one typed registry. Their
/// implementations stay grouped by feature in `file_ops` and `native_views`.
pub(super) fn native_registry() -> &'static NativeProviderRegistry<dyn ContextMenuProvider> {
    static REGISTERED: OnceLock<NativeProviderRegistry<dyn ContextMenuProvider>> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let mut providers: NativeProviderRegistry<dyn ContextMenuProvider> =
            NativeProviderRegistry::new(ProviderKind::ContextMenu);
        file_ops::register(&mut providers);
        native_views::register(&mut providers);
        providers
    })
}

fn provider_for(
    resolver: &Resolver,
    directory: &str,
    resources: &Arc<Vec<ResourceSnapshot>>,
) -> (Arc<dyn ContextMenuProvider>, Option<Arc<Mutex<Option<String>>>>) {
    match resolver.plugin.as_deref() {
        None => (
            native_registry().get_arc(&resolver.id).unwrap_or_else(|| {
                Arc::new(FailedMenuProvider(
                    "registered native ContextMenuProvider disappeared".into(),
                ))
            }),
            None,
        ),
        Some(plugin) => {
            let proxy = proxy::ScopedMenuProxy::new(
                plugin.clone(),
                resolver.id.clone(),
                directory.into(),
                resources.clone(),
            );
            match proxy {
                Ok(proxy) => {
                    let remote_session = proxy.remote_session_slot();
                    (Arc::new(proxy), Some(remote_session))
                }
                Err(error) => (Arc::new(FailedMenuProvider(error)), None),
            }
        }
    }
}

struct FailedMenuProvider(String);
impl ContextMenuProvider for FailedMenuProvider {
    fn resolve(
        &self,
        _ctx: &dyn fiex_provider_contracts::ProviderInvocation,
    ) -> Result<Vec<MenuItem>, fiex_provider_contracts::ProviderError> {
        Err(fiex_provider_contracts::ProviderError::internal(self.0.clone()))
    }

    fn execute(
        &self,
        _ctx: &dyn fiex_provider_contracts::ProviderInvocation,
    ) -> Result<MenuExecuteResult, fiex_provider_contracts::ProviderError> {
        Err(fiex_provider_contracts::ProviderError::internal(self.0.clone()))
    }
}

fn validate_plugin_action(
    plugin: &crate::plugins::host::PluginInfo,
    result: &mut MenuExecuteResult,
) -> Result<(), String> {
    match result {
        MenuExecuteResult::CoreAction(_)
        | MenuExecuteResult::ConnectionCoreAction(_)
        | MenuExecuteResult::RemoteAction(_)
        | MenuExecuteResult::NativeAction(_) => {
            Err("plugin returned a reserved action kind".into())
        }
        MenuExecuteResult::Panel(panel) => {
            let entry = panel.entry.as_deref().ok_or("panel result missing entry")?;
            let asset = crate::plugins::host::plugin_view_asset(&plugin.id, entry)?;
            panel.entry = None;
            panel.asset_path = Some(asset);
            Ok(())
        }
        _ => Ok(()),
    }
}

fn execute_provider(
    resolver: &Resolver,
    implementation: &dyn ContextMenuProvider,
    invocation: &dyn fiex_provider_contracts::ProviderInvocation,
) -> Result<MenuExecuteResult, String> {
    let mut result = implementation
        .execute(invocation)
        .map_err(|error| error.to_string())?;
    if let Some(plugin) = resolver.plugin.as_deref() {
        validate_plugin_action(plugin, &mut result)?;
    }
    Ok(result)
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
        providers.retain(|provider| provider.plugin.is_none());
    }
    let request = ProviderRequest {
        method: "contextMenu.resolve".into(),
        params: json!({}),
    };
    let mut remote_sessions = HashMap::new();
    let providers = providers
        .into_iter()
        .map(|resolver| {
            let (implementation, remote_session) = provider_for(&resolver, directory, &resources);
            if let Some(remote_session) = remote_session {
                remote_sessions.insert(resolver.id.clone(), remote_session);
            }
            ProviderCandidate {
                resolver,
                implementation,
            }
        })
        .collect();
    let invocation = file_ops::invocation(&ctx, &request.method, request.params.clone());
    Ok(EventRouter::resolve_shared(
        "contextMenu.resolve",
        providers,
        &invocation,
        |resolver, provider, invocation| {
            let items = provider
                .resolve(invocation)
                .map_err(|error| error.to_string())?;
            let mut result = json!({"items":items});
            if resolver.plugin.is_some() {
                let session = remote_sessions
                    .get(&resolver.id)
                    .ok_or("plugin menu session unavailable")?
                    .lock()
                    .map_err(|_| "plugin menu session unavailable")?
                    .clone()
                    .ok_or("plugin menu session ID is missing")?;
                result["menuSessionId"] = json!(session);
            }
            Ok(result)
        },
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
        if session.provider.plugin.is_none() {
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
    let (implementation, remote_session) = provider_for(&provider, &directory, &resources);
    let candidate = ProviderCandidate {
        resolver: provider.clone(),
        implementation,
    };
    let invocation = NativeProviderInvocation::new(
        request.method.clone(),
        request.params.clone(),
        resources.as_slice(),
        None,
    );
    let result = EventRouter::resolve_shared(
        "contextMenu.resolve",
        vec![candidate],
        &invocation,
        |resolver, implementation, invocation| {
            let items = implementation
                .resolve(invocation)
                .map_err(|error| error.to_string())?;
            let mut result = json!({"items":items});
            if resolver.plugin.is_some() {
                let session = remote_session
                    .as_ref()
                    .ok_or("plugin menu session unavailable")?
                    .lock()
                    .map_err(|_| "plugin menu session unavailable")?
                    .clone()
                    .ok_or("plugin menu session ID is missing")?;
                result["menuSessionId"] = json!(session);
            }
            Ok(result)
        },
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
    let (implementation, _) = provider_for(
        &session.provider,
        &session.directory,
        &session.resources,
    );
    let candidate = ProviderCandidate {
        resolver: session.provider.clone(),
        implementation,
    };
    let invocation = file_ops::invocation(&ctx, &request.method, request.params.clone());
    let result = EventRouter::resolve_shared(
        "contextMenu.execute",
        vec![candidate],
        &invocation,
        |resolver, provider, invocation| execute_provider(resolver, provider, invocation),
        |_, result| {
            let wire = serde_json::to_value(&result).map_err(|error| error.to_string())?;
            crate::plugins::events::validate_result("contextMenu.execute", &wire)?;
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
    )?;
    serde_json::to_value(result).map_err(|error| error.to_string())
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
