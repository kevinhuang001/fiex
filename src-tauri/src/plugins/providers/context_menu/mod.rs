//! Shared EventRouter pipeline for builtin and plugin menus.
mod actions;
mod builtin;
mod commands;
mod connection_cache;
mod file_ops;
mod native_views;
pub(crate) mod open_with;
#[cfg(feature = "desktop")]
pub(crate) use open_with::{open_picker_choice, picker_candidates, PickerResponse};
mod proxy;
mod session;
pub use actions::execute_remote;
pub use commands::{children, execute};
#[cfg(test)]
mod tests;
mod validation;

pub(crate) fn forget_plugin(plugin_id: &str) {
    proxy::forget_plugin(plugin_id);
    if let Ok(mut sessions) = sessions().lock() {
        sessions.retain(|_, session| {
            session
                .provider
                .plugin
                .as_deref()
                .is_none_or(|plugin| plugin.id != plugin_id)
        });
    }
}

use crate::plugins::events::{EventRouter, ProviderCandidate, Resolver};
use crate::plugins::{
    native_invocation::NativeProviderInvocation, provider_registry::native::NativeProviderRegistry,
    resource::resource_context::ResourceSnapshot,
};
use fiex_provider_contracts::{
    ContextMenuProvider, MenuExecuteResult, MenuItem, ProviderKind, ProviderRequest,
};
use serde_json::{json, Value};
#[cfg(test)]
use session::session_ids;
use session::{allowed, register, sessions, MenuScope};
use std::{
    collections::HashMap,
    sync::{Arc, Condvar, Mutex, OnceLock},
    time::{Duration, Instant},
};
use validation::validated_submenu;

/// Add all Core-owned menu modules to the common Runtime registry.
pub(crate) fn register_native(values: &mut NativeProviderRegistry<dyn ContextMenuProvider>) {
    file_ops::register(values);
    native_views::register(values);
}

/// Each caller gets a lightweight typed view over the already initialized
/// Runtime registry; provider instances live in the shared registry.
pub(super) fn native_registry() -> NativeProviderRegistry<dyn ContextMenuProvider> {
    crate::plugins::providers::initialize_native_providers();
    NativeProviderRegistry::shared(ProviderKind::ContextMenu)
}

type ResolvedProvider = (
    Arc<dyn ContextMenuProvider>,
    Option<Arc<Mutex<Option<String>>>>,
);

fn provider_for(
    resolver: &Resolver,
    directory: &str,
    resources: &Arc<Vec<ResourceSnapshot>>,
) -> ResolvedProvider {
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
        Err(fiex_provider_contracts::ProviderError::internal(
            self.0.clone(),
        ))
    }

    fn execute(
        &self,
        _ctx: &dyn fiex_provider_contracts::ProviderInvocation,
    ) -> Result<MenuExecuteResult, fiex_provider_contracts::ProviderError> {
        Err(fiex_provider_contracts::ProviderError::internal(
            self.0.clone(),
        ))
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
    query_scope(directory, paths, MenuScope::All, None)
}

/// Top-level, Core-owned items. This path must stay free of plugin IPC so the
/// first visible menu is complete and cannot resize while plugins respond.
pub fn query_core(
    directory: &str,
    paths: &[String],
    session_id: Option<&str>,
) -> Result<Vec<Value>, String> {
    query_scope(directory, paths, MenuScope::Core, session_id)
}

/// Plugin-owned items are resolved independently from the Core top-level menu.
pub fn query_plugins(
    directory: &str,
    paths: &[String],
    session_id: Option<&str>,
) -> Result<Vec<Value>, String> {
    query_scope(directory, paths, MenuScope::Plugins, session_id)
}

const MENU_CONTEXT_TTL: Duration = Duration::from_secs(10);
const MENU_CONTEXT_LIMIT: usize = 512;

struct MenuContextFlight {
    result: Mutex<Option<Result<builtin::Context, String>>>,
    ready: Condvar,
}

enum MenuContextEntry {
    Loading(Arc<MenuContextFlight>),
    Ready {
        created: Instant,
        context: Box<builtin::Context>,
    },
}

fn menu_contexts() -> &'static Mutex<HashMap<String, MenuContextEntry>> {
    static CONTEXTS: OnceLock<Mutex<HashMap<String, MenuContextEntry>>> = OnceLock::new();
    CONTEXTS.get_or_init(Default::default)
}

/// Core and plugin requests from one right-click share the same resource
/// snapshot, so local Stat and VFS metadata discovery run only once.
fn menu_context(
    directory: &str,
    paths: &[String],
    session_id: Option<&str>,
) -> Result<builtin::Context, String> {
    let Some(session_id) = session_id.filter(|value| {
        !value.is_empty()
            && value.len() <= 128
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    }) else {
        return builtin::Context::load_for_menu(directory, paths, false);
    };
    let selection = serde_json::to_string(paths).map_err(|error| error.to_string())?;
    let key = format!("{session_id}\0{directory}\0{selection}");
    let (flight, load) = {
        let mut contexts = menu_contexts()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        contexts.retain(|_, entry| match entry {
            MenuContextEntry::Loading(_) => true,
            MenuContextEntry::Ready { created, .. } => created.elapsed() < MENU_CONTEXT_TTL,
        });
        if let Some(entry) = contexts.get(&key) {
            match entry {
                MenuContextEntry::Ready { context, .. } => return Ok(context.as_ref().clone()),
                MenuContextEntry::Loading(flight) => (Arc::clone(flight), false),
            }
        } else {
            if contexts.len() >= MENU_CONTEXT_LIMIT {
                return Err("too many active context-menu snapshots".into());
            }
            let flight = Arc::new(MenuContextFlight {
                result: Mutex::new(None),
                ready: Condvar::new(),
            });
            contexts.insert(key.clone(), MenuContextEntry::Loading(Arc::clone(&flight)));
            (flight, true)
        }
    };
    if load {
        let result = builtin::Context::load_for_menu(directory, paths, false);
        {
            let mut contexts = menu_contexts()
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if let Ok(context) = &result {
                contexts.insert(
                    key,
                    MenuContextEntry::Ready {
                        created: Instant::now(),
                        context: Box::new(context.clone()),
                    },
                );
            } else {
                contexts.remove(&key);
            }
        }
        *flight
            .result
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(result.clone());
        flight.ready.notify_all();
        result
    } else {
        let mut result = flight
            .result
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        while result.is_none() {
            result = flight
                .ready
                .wait(result)
                .unwrap_or_else(|error| error.into_inner());
        }
        result.as_ref().expect("context flight completed").clone()
    }
}

fn query_scope(
    directory: &str,
    paths: &[String],
    scope: MenuScope,
    session_id: Option<&str>,
) -> Result<Vec<Value>, String> {
    // An empty selection is the blank canvas. It uses the same provider
    // pipeline; manifests opt in with `match.selection: "zero"` (or `any`).
    if paths.len() > 256 || directory.is_empty() {
        return Err("invalid menu selection".into());
    }
    let mut ctx = menu_context(directory, paths, session_id)?;
    if scope != MenuScope::Plugins {
        ctx.preflight = file_ops::preflight_availability(&ctx);
    }
    let resources = Arc::clone(&ctx.resources);
    let native = if scope == MenuScope::Plugins {
        Vec::new()
    } else {
        let mut candidates = builtin::providers(&ctx);
        candidates.extend(native_views::providers_for_context(&ctx));
        candidates
    };
    let mut providers = crate::plugins::provider_registry::candidates(
        "contextMenu.resolve",
        &ctx.match_targets,
        directory,
        native,
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
    let menus = EventRouter::resolve_shared(
        "contextMenu.resolve",
        providers,
        &invocation,
        |resolver, provider, _invocation| {
            let bound = if resolver.plugin.is_none() {
                file_ops::invocation_for(
                    &resolver.id,
                    &ctx,
                    &request.method,
                    request.params.clone(),
                )?
            } else {
                file_ops::invocation(&ctx, &request.method, request.params.clone())
            };
            let items = provider
                .resolve(&bound)
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
    );
    Ok(menus)
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
