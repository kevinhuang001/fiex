//! Core system-event coordinator. Custom business RPC stays on plugin.invoke.
use crate::plugin_resource_context::ResourceSnapshot;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
#[path = "events/matching.rs"]
mod matching;

#[derive(Clone)]
pub enum Handler {
    Builtin(&'static str),
    Plugin(Box<crate::plugin_host::PluginInfo>),
}
#[derive(Clone)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub priority: i64,
    pub handler: Handler,
}
impl Provider {
    pub fn builtin(id: &'static str, priority: i64) -> Self {
        Self {
            id: id.into(),
            name: "Fiex".into(),
            priority,
            handler: Handler::Builtin(id),
        }
    }
    pub fn json(&self) -> Value {
        json!({"id":self.id,"provider":self.name,"priority":self.priority})
    }
}

fn extension_for(method: &str) -> Option<&'static str> {
    match method {
        "info.resolve" => Some("info"),
        "preview.open" => Some("preview"),
        "decorations.resolve" => Some("decorations"),
        "contextMenu.resolve" | "contextMenu.execute" => Some("contextMenus"),
        "search.query" => Some("search"),
        "toolbar.invoke" => Some("toolbar"),
        "hooks.invoke" => Some("hooks"),
        "connection.open" | "connection.create" | "connection.validate" => Some("connections"),
        _ => None,
    }
}

pub struct EventRouter;
impl EventRouter {
    pub fn providers(
        method: &str,
        metadata: &[Value],
        directory: &str,
        mut providers: Vec<Provider>,
    ) -> Vec<Provider> {
        let Some(feature) = extension_for(method) else {
            return providers;
        };
        for plugin in crate::plugin_host::catalog_matching(|manifest| {
            matching::matches(method, &manifest.extensions, metadata, directory)
        }) {
            if !plugin.enabled || !plugin.approved || !plugin.backend_available {
                continue;
            }
            let Some(contribution) = plugin.extensions.get(feature) else {
                continue;
            };
            if !contribution.is_object() && contribution.as_bool() != Some(true) {
                continue;
            }
            let declared = plugin.declared_permissions.iter().cloned().collect();
            let policy =
                crate::plugin_permission_policy::PermissionPolicy::new(&plugin.id, &declared);
            let required = match method {
                "decorations.resolve" => {
                    Some(crate::plugin_permission_policy::Capability::FoldersRead)
                }
                "hooks.invoke" => Some(crate::plugin_permission_policy::Capability::Hooks),
                _ => None,
            };
            if required.is_some_and(|capability| !policy.allowed(capability)) {
                continue;
            }
            providers.push(Provider {
                id: format!("plugin:{}", plugin.id),
                name: plugin.name.clone(),
                priority: contribution
                    .get("priority")
                    .and_then(Value::as_i64)
                    .unwrap_or(50),
                handler: Handler::Plugin(Box::new(plugin)),
            });
        }
        providers.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.id.cmp(&b.id)));
        providers
    }
    /// Every provider gets its own invocation/authorization. Validators admit
    /// one provider atomically; the feature supplies only its combination rule.
    pub fn resolve<T, R>(
        method: &str,
        providers: Vec<Provider>,
        mut builtin: impl FnMut(&Provider, &str) -> Result<Value, String>,
        mut plugin: impl FnMut(&crate::plugin_host::PluginInfo) -> Result<Value, String>,
        mut validate: impl FnMut(&Provider, Value) -> Result<T, String>,
        mut supported: impl FnMut(&T) -> bool,
        collect: impl FnOnce(Vec<(Provider, Result<T, String>)>) -> R,
    ) -> R {
        let first = method == "preview.open" || method == "hooks.invoke";
        let exclusive = method.starts_with("connection.")
            || method.starts_with("vfs.")
            || matches!(method, "toolbar.invoke" | "contextMenu.execute");
        let mut outcomes = Vec::new();
        for provider in providers {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let value = match &provider.handler {
                    Handler::Builtin(id) => builtin(&provider, id)?,
                    Handler::Plugin(p) => plugin(p)?,
                };
                validate_result(method, &value)?;
                validate(&provider, value)
            }))
            .unwrap_or_else(|_| Err("provider panicked".into()));
            let accepted = result.as_ref().is_ok_and(&mut supported);
            outcomes.push((provider, result));
            if exclusive || (first && accepted) {
                break;
            }
        }
        collect(outcomes)
    }
}

pub fn validate_result(method: &str, value: &Value) -> Result<(), String> {
    if serde_json::to_vec(value).map_err(|e| e.to_string())?.len() > 4 * 1024 * 1024 {
        return Err("provider result exceeds limit".into());
    }
    let array = |key: &str, limit: usize| {
        value
            .get(key)
            .and_then(Value::as_array)
            .filter(|a| a.len() <= limit)
            .ok_or_else(|| format!("invalid {key} result"))
    };
    match method {
        "info.resolve" => {
            let sections = array("sections", 64)?;
            let mut ids = std::collections::HashSet::new();
            for s in sections {
                for key in ["id", "title"] {
                    if s[key]
                        .as_str()
                        .filter(|s| !s.is_empty() && s.len() <= 1024)
                        .is_none()
                    {
                        return Err("invalid info section".into());
                    }
                }
                if !ids.insert(s["id"].as_str().unwrap()) {
                    return Err("duplicate info section id".into());
                }
                let rows = s["rows"]
                    .as_array()
                    .filter(|a| a.len() <= 256)
                    .ok_or("invalid info rows")?;
                for r in rows {
                    if r["label"].as_str().filter(|s| s.len() <= 16384).is_none()
                        || r["value"].as_str().filter(|s| s.len() <= 16384).is_none()
                        || r.get("copyable").is_some_and(|v| !v.is_boolean())
                    {
                        return Err("invalid info row".into());
                    }
                }
            }
        }
        "preview.open" => {
            if !matches!(
                value["kind"].as_str(),
                Some("html" | "text" | "image" | "media" | "pdf" | "fallback" | "unsupported")
            ) {
                return Err("invalid preview kind".into());
            }
        }
        "contextMenu.resolve" => {
            array("items", 100)?;
        }
        "decorations.resolve" => {
            array("decorations", 100_000)?;
        }
        "search.query" => {
            array("entries", 500)?;
        }
        "hooks.invoke" => {
            if value["handled"].as_bool().is_none()
                || value.get("preview").is_some_and(|v| !v.is_boolean())
            {
                return Err("invalid hook response".into());
            }
        }
        _ => {
            if !value.is_object() && !value.is_null() {
                return Err("invalid event result".into());
            }
        }
    }
    Ok(())
}

/// One shared request preparation path, at the host boundary. No business
/// module may grant privileges by supplying a reference or a read token.
pub fn prepare(
    plugin: &str,
    method: &str,
    mut params: Value,
    scope: &mut Option<crate::plugin_fs_access::Scope>,
    declared: &std::collections::BTreeSet<String>,
) -> Result<Value, String> {
    if extension_for(method).is_none() {
        return Ok(params);
    }
    if !params.is_object() {
        return Err("event parameters must be an object".into());
    }
    let policy = crate::plugin_permission_policy::PermissionPolicy::new(plugin, declared);
    let metadata_allowed = policy.allowed(crate::plugin_permission_policy::Capability::Metadata);
    let discovery = method == "contextMenu.resolve";
    if let Some(scope) = scope {
        if discovery {
            scope.restrict_to_metadata();
        }
        let paths = scope.selected_paths();
        if params.get("targets").is_none() {
            params["targets"] = json!(paths
                .iter()
                .map(|path| ResourceSnapshot::new(&path.to_string_lossy(), path)
                    .project(plugin, "", true, false))
                .collect::<Vec<_>>());
        }
        let targets = params["targets"]
            .as_array_mut()
            .filter(|a| a.len() <= 5000)
            .ok_or("invalid event targets")?;
        for (index, target) in targets.iter_mut().enumerate() {
            if !scope.resource_bound(&target["ref"])? {
                scope.bind_resource(
                    target,
                    if discovery {
                        None
                    } else {
                        paths.get(index).map(PathBuf::as_path)
                    },
                )?;
            }
            if let Some(object) = target.as_object_mut() {
                object.remove("read");
                if !metadata_allowed {
                    object.remove("metadata");
                }
            }
        }
    } else if params.get("targets").is_none() {
        params["targets"] = json!([]);
    }
    Ok(params)
}

/// Core-owned system events are not exposed through the plugin UI bridge.
/// Unknown methods remain legal plugin-local business methods.
pub fn is_protected_method(method: &str) -> bool {
    extension_for(method).is_some()
        || [
            "host.",
            "core.",
            "plugin.",
            "contextMenu.",
            "vfs.",
            "fs.",
            "connection.",
        ]
        .iter()
        .any(|prefix| method.starts_with(prefix))
}

fn require_event(method: &str) -> Result<(), String> {
    if extension_for(method).is_some() {
        Ok(())
    } else {
        Err(format!("unregistered Core event: {method}"))
    }
}

/// Recheck a stored menu session using its actual targets, never empty placeholders.
pub fn menu_allowed(plugin_id: &str, resources: &[ResourceSnapshot], directory: &str) -> bool {
    EventRouter::providers(
        "contextMenu.resolve",
        &matching::metadata(resources),
        directory,
        Vec::new(),
    )
    .iter()
    .any(|provider| matches!(&provider.handler, Handler::Plugin(plugin) if plugin.id == plugin_id))
}

pub fn call_owner(
    method: &str,
    plugin_id: &str,
    params: Value,
    resources: &[ResourceSnapshot],
    directory: Option<&str>,
) -> Result<Value, String> {
    let metadata = matching::metadata(resources);
    let providers = EventRouter::providers(method, &metadata, directory.unwrap_or(""), Vec::new())
        .into_iter()
        .filter(|p| matches!(&p.handler,Handler::Plugin(plugin) if plugin.id==plugin_id))
        .collect();
    EventRouter::resolve(
        method,
        providers,
        |_, _| Err("builtin owner not registered".into()),
        |plugin| invoke_resources(plugin, method, params.clone(), resources, directory),
        |_, value| Ok(value),
        |_| true,
        |results| {
            results
                .into_iter()
                .next()
                .ok_or_else(|| "event owner unavailable or does not match targets".to_string())?
                .1
        },
    )
}

pub fn invoke(plugin: &str, method: &str, params: Value) -> Result<Value, String> {
    require_event(method)?;
    let result = crate::plugin_host::global().invoke(plugin, method, params)?;
    validate_result(method, &result)?;
    Ok(result)
}
pub fn invoke_prepared(
    plugin: &str,
    method: &str,
    params: Value,
    scope: crate::plugin_fs_access::Scope,
) -> Result<Value, String> {
    require_event(method)?;
    let value = crate::plugin_host::global().invoke_scoped(plugin, method, params, Some(scope))?;
    validate_result(method, &value)?;
    Ok(value)
}

/// A user-initiated open hook or context-menu action may need to create its
/// result next to the selection ("extract here", "compress here"). Only those
/// explicit events get the containing folder, and only when the manifest
/// declares a create capability; every write still passes the capability
/// check in host dispatch, so a read-only plugin gains nothing.
fn action_folders(
    method: &str,
    directory: Option<&str>,
    paths: &[PathBuf],
    declared: &std::collections::BTreeSet<String>,
) -> Result<Vec<PathBuf>, String> {
    if !matches!(method, "hooks.invoke" | "contextMenu.execute")
        || !["files.create", "folders.create"]
            .iter()
            .any(|name| declared.contains(*name))
    {
        return Ok(Vec::new());
    }
    let mut folders = Vec::new();
    if let Some(folder) = directory.filter(|value| !crate::mounts::handles(value)) {
        folders.push(crate::plugin_fs_access::canonical_selection(Path::new(
            folder,
        ))?);
    }
    folders.extend(
        paths
            .iter()
            .filter_map(|path| path.parent().map(Path::to_path_buf)),
    );
    Ok(folders)
}

/// Resources are resolved before provider matching and reused by all calls.
/// Only a provider with approved content IO can cause remote materialization.
pub fn resource_scope(
    plugin: &crate::plugin_host::PluginInfo,
    method: &str,
    mut params: Value,
    resources: &[ResourceSnapshot],
    directory: Option<&str>,
) -> Result<(Value, crate::plugin_fs_access::Scope), String> {
    require_event(method)?;
    if !matching::matches_resources(method, &plugin.extensions, resources, directory) {
        return Err("provider does not match event targets".into());
    }
    let declared = plugin.declared_permissions.iter().cloned().collect();
    let policy = crate::plugin_permission_policy::PermissionPolicy::new(&plugin.id, &declared);
    let content = !matches!(
        method,
        "contextMenu.resolve" | "decorations.resolve" | "search.query"
    ) && (policy.allowed(crate::plugin_permission_policy::Capability::FilesRead)
        || method == "contextMenu.execute"
        || method == "toolbar.invoke");
    params["targets"] = json!(resources
        .iter()
        .map(|r| r.project(
            &plugin.id,
            "",
            policy.allowed(crate::plugin_permission_policy::Capability::Metadata),
            false
        ))
        .collect::<Vec<_>>());
    let mut paths = resources
        .iter()
        .filter(|r| !crate::mounts::handles(&r.display))
        .map(|r| crate::plugin_fs_access::canonical_selection(&r.local))
        .collect::<Result<Vec<_>, _>>()?;
    for folder in action_folders(method, directory, &paths, policy.declared())? {
        if !paths.contains(&folder) {
            paths.push(folder);
        }
    }
    if policy.allowed(crate::plugin_permission_policy::Capability::Metadata) {
        for (target, resource) in params["targets"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .zip(resources)
        {
            if target["metadata"].is_object() {
                target["metadata"]["readonly"] = json!(resource.read_only);
            }
        }
    }
    // Use exact files for content actions. Metadata-only remote targets are
    // backed by an empty local capability, plus operation-owned attributes.
    let mut scope = if !paths.is_empty() {
        let mut root = paths[0].parent().unwrap_or(Path::new(".")).to_path_buf();
        for path in &paths {
            while !path.starts_with(&root) {
                root = root
                    .parent()
                    .ok_or("no common operation root")?
                    .to_path_buf();
            }
        }
        crate::plugin_fs_access::Scope::selected(&root, &paths)?
    } else {
        crate::plugin_fs_access::Scope::selected(&std::env::temp_dir(), &[])?
    };
    let directory_event = matches!(method, "decorations.resolve" | "search.query");
    if method == "contextMenu.resolve" || (!content && !directory_event) {
        scope.restrict_to_metadata();
    }
    for (resource, target) in resources.iter().zip(params["targets"].as_array().unwrap()) {
        let local = if !crate::mounts::handles(&resource.display) {
            Some(crate::plugin_fs_access::canonical_selection(
                &resource.local,
            )?)
        } else {
            None
        };
        scope.bind_resource(target, local.as_deref())?;
        if (content || directory_event) && crate::mounts::handles(&resource.display) {
            scope.bind_remote(target, resource)?;
        }
    }
    // Directory events explicitly authorize their requested root, not an unrelated current pane.
    if matches!(method, "decorations.resolve" | "search.query") {
        if let Some(dir) = directory.filter(|d| !crate::mounts::handles(d)) {
            scope = crate::plugin_fs_access::Scope::new(Path::new(dir))?;
            for (r, target) in resources.iter().zip(params["targets"].as_array().unwrap()) {
                scope.bind_resource(target, Some(&r.local))?;
            }
        }
    }
    Ok((params, scope))
}

pub fn invoke_resources(
    plugin: &crate::plugin_host::PluginInfo,
    method: &str,
    params: Value,
    resources: &[ResourceSnapshot],
    directory: Option<&str>,
) -> Result<Value, String> {
    let (params, scope) = resource_scope(plugin, method, params, resources, directory)?;
    let result =
        crate::plugin_host::global().invoke_scoped(&plugin.id, method, params, Some(scope))?;
    validate_result(method, &result)?;
    Ok(result)
}

#[cfg(test)]
#[path = "events/tests.rs"]
mod tests;
