//! Menu session storage and lifecycle.
use super::{
    builtin,
    validation::{root_menu_valid, submenu_depths},
};
use crate::plugins::{events::Resolver, resource::resource_context::ResourceSnapshot};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, OnceLock},
    time::Instant,
};

pub(super) struct Session {
    pub(super) provider: Resolver,
    /// Remote SDK session is private to its owning plugin; never use it as the
    /// Core/UI session key or allow it to overwrite another provider's state.
    pub(super) remote_session_id: Option<String>,
    pub(super) directory: String,
    pub(super) paths: Vec<String>,
    pub(super) resources: Arc<Vec<ResourceSnapshot>>,
    pub(super) actions: HashSet<String>,
    pub(super) submenus: HashMap<String, usize>,
    pub(super) created: Instant,
}
/// Menu resolution is split by owner so the top-level Core menu never waits for
/// plugin IPC. `All` is kept for callers that explicitly want one batch.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum MenuScope {
    All,
    Core,
    Plugins,
}
pub(super) fn sessions() -> &'static Mutex<HashMap<String, Session>> {
    static SESSIONS: OnceLock<Mutex<HashMap<String, Session>>> = OnceLock::new();
    SESSIONS.get_or_init(Default::default)
}
pub(super) fn allowed(session: &Session) -> bool {
    match session.provider.plugin.as_deref() {
        None => true,
        Some(plugin) => crate::plugins::events::menu_allowed(
            &plugin.id,
            &session.provider.id,
            &session.resources,
            &session.directory,
        ),
    }
}
/// Generate an independent Core session ID for every menu, regardless of
/// which provider created it. Only IPC providers retain their transport token.
pub(super) fn session_ids(
    result: &Value,
    is_plugin: bool,
) -> Result<(String, Option<String>), String> {
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

pub(super) fn register(
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
    let (session_id, remote_session_id) = session_ids(&result, provider.plugin.is_some())?;
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
    match provider.plugin.as_deref() {
        None => Ok(
            json!({"kind":"builtin","providerId":provider.id,"menuSessionId":session_id,"items":items}),
        ),
        Some(plugin) => Ok(
            json!({"kind":"plugin","providerId":provider.id,"pluginId":plugin.id,"menuSessionId":session_id,"items":items}),
        ),
    }
}
