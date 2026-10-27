//! Shared EventRouter pipeline for builtin and plugin menus.
mod actions;
mod builtin;
pub use actions::execute_remote;
#[cfg(test)]
mod tests;
mod validation;

use crate::plugin_events::{EventRouter, Handler, Provider};
use crate::plugin_resource_context::ResourceSnapshot;
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, OnceLock},
    time::Instant,
};
use validation::{root_menu_valid, submenu_depths, validated_submenu};

struct Session {
    provider: Provider,
    directory: String,
    paths: Vec<String>,
    resources: Arc<Vec<ResourceSnapshot>>,
    actions: HashSet<String>,
    submenus: HashMap<String, usize>,
    created: Instant,
}
fn sessions() -> &'static Mutex<HashMap<String, Session>> {
    static SESSIONS: OnceLock<Mutex<HashMap<String, Session>>> = OnceLock::new();
    SESSIONS.get_or_init(Default::default)
}
fn allowed(session: &Session) -> bool {
    match &session.provider.handler {
        Handler::Builtin(_) => true,
        Handler::Plugin(plugin) => {
            crate::plugin_events::menu_allowed(&plugin.id, &session.resources, &session.directory)
        }
    }
}
fn register(
    provider: &Provider,
    ctx: &builtin::Context,
    resources: &Arc<Vec<ResourceSnapshot>>,
    result: Value,
) -> Result<Value, String> {
    let mut actions = root_menu_valid(&result)?;
    let items = result["items"].as_array().ok_or("invalid menu items")?;
    if items.is_empty() {
        return Ok(json!({"items":[]}));
    }
    for item in items {
        if item["disabled"] == true {
            if let Some(id) = item["id"].as_str() {
                actions.remove(id);
            }
        }
    }
    let session_id = result["menuSessionId"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or(crate::mounts::new_id()?);
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
    if paths.is_empty() || paths.len() > 256 || directory.is_empty() {
        return Err("invalid menu selection".into());
    }
    let ctx = builtin::Context::load(directory, paths)?;
    let resources = Arc::new(
        ctx.entries
            .iter()
            .map(|entry| {
                let mut snapshot = ResourceSnapshot::from_metadata(
                    &entry.path,
                    ResourceSnapshot::entry_metadata(entry),
                );
                snapshot.read_only = entry.readonly;
                snapshot
            })
            .collect::<Vec<_>>(),
    );
    let metadata = resources
        .iter()
        .map(|resource| serde_json::to_value(resource.metadata()).unwrap_or_default())
        .collect::<Vec<_>>();
    let providers = EventRouter::providers(
        "contextMenu.resolve",
        &metadata,
        directory,
        builtin::providers(),
    );
    Ok(EventRouter::resolve(
        "contextMenu.resolve",
        providers,
        |_, id| builtin::resolve(id, &ctx),
        |plugin| {
            let session_id = crate::mounts::new_id()?;
            let mut result = crate::plugin_events::invoke_resources(
                plugin,
                "contextMenu.resolve",
                json!({"menuSessionId":session_id,"interactionId":crate::mounts::new_id()?}),
                &resources,
                Some(directory),
            )?;
            result["menuSessionId"] = json!(session_id);
            Ok(result)
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
    let (plugin_id, directory, resources, depth) = {
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
            session.directory.clone(),
            session.resources.clone(),
            *session.submenus.get(parent_id).ok_or("unknown submenu")?,
        )
    };
    let result = crate::plugin_events::call_owner(
        "contextMenu.resolve",
        &plugin_id,
        json!({"menuSessionId":session_id,"parentId":parent_id,"interactionId":crate::mounts::new_id()?}),
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
    EventRouter::resolve(
        "contextMenu.execute",
        vec![session.provider.clone()],
        |_, id| {
            builtin::execute(
                id,
                item_id,
                &builtin::Context::load(&session.directory, &session.paths)?,
            )
        },
        |plugin| {
            let params = json!({"menuSessionId":session_id,"itemId":item_id,"interactionId":crate::mounts::new_id()?});
            let mut result = crate::plugin_events::call_owner(
                "contextMenu.execute",
                &plugin.id,
                params,
                &session.resources,
                Some(&session.directory),
            )?;
            if matches!(result["kind"].as_str(), Some("coreAction" | "remoteAction")) {
                return Err("plugin returned a reserved action kind".into());
            }
            if result["kind"] == "panel" {
                let entry = result["entry"]
                    .as_str()
                    .ok_or("panel result missing entry")?;
                let asset = crate::plugin_host::plugin_view_asset(&plugin.id, entry)?;
                let object = result.as_object_mut().ok_or("invalid panel result")?;
                object.remove("entry");
                object.insert("assetPath".into(), json!(asset));
            }
            Ok(result)
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
