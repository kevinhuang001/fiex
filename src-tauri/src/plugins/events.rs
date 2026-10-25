//! Common provider discovery, request envelope and operation-bound dispatch.
use serde_json::{json, Value};
use std::path::Path;

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
/// Stateless coordinator; features supply only handlers and their result rules.
pub struct EventRouter;

impl EventRouter {
    pub fn providers(
        method: &str,
        metadata: &[Value],
        directory: &str,
        mut builtins: Vec<Provider>,
    ) -> Vec<Provider> {
        let feature = match method {
            "info.resolve" => "info",
            "preview.open" => "preview",
            "contextMenu.resolve" | "contextMenu.execute" => "contextMenus",
            "decorations.resolve" => "decorations",
            "search.query" => "search",
            "toolbar.invoke" => "toolbar",
            "connection.open" => "connections",
            _ => return builtins,
        };
        for plugin in crate::plugin_host::catalog() {
            if !plugin.enabled || !plugin.approved || !plugin.backend_available {
                continue;
            }
            let Some(contribution) = plugin
                .extensions
                .get(feature)
                .filter(|v| v.is_object() || **v == Value::Bool(true))
            else {
                continue;
            };
            let rules = crate::plugin_match::MatchRules::from_contribution(contribution);
            if !rules.matches_selection(metadata.len())
                || (!directory.is_empty() && !rules.matches_contains(directory))
            {
                continue;
            }
            if !metadata.is_empty()
                && !metadata.iter().any(|value| {
                    rules.matches(
                        value["extension"].as_str().unwrap_or(""),
                        value["mime"].as_str().unwrap_or(""),
                        value["kind"] == "directory" || value["isDir"].as_bool().unwrap_or(false),
                    )
                })
            {
                continue;
            }
            builtins.push(Provider {
                id: format!("plugin:{}", plugin.id),
                name: plugin.name.clone(),
                priority: contribution
                    .get("priority")
                    .and_then(Value::as_i64)
                    .unwrap_or(50),
                handler: Handler::Plugin(Box::new(plugin)),
            });
        }
        builtins.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.id.cmp(&b.id)));
        builtins
    }

    /// Validation happens per provider before the event policy admits its result.
    /// Merge events retain all successful sources; Preview stops at the first
    /// supported result; owner-routed events execute exactly one provider.
    pub fn resolve<T>(
        method: &str,
        providers: Vec<Provider>,
        mut builtin: impl FnMut(&Provider, &str) -> Result<Value, String>,
        mut plugin: impl FnMut(&crate::plugin_host::PluginInfo) -> Result<Value, String>,
        mut validate: impl FnMut(&Provider, Value) -> Result<T, String>,
        mut supported: impl FnMut(&T) -> bool,
    ) -> Vec<(Provider, Result<T, String>)> {
        let first = method == "preview.open";
        let exclusive = method.starts_with("connection.")
            || method.starts_with("vfs.")
            || matches!(method, "toolbar.invoke" | "contextMenu.execute");
        let mut results = Vec::new();
        for provider in providers {
            let result = dispatch(
                &provider,
                |id| builtin(&provider, id),
                |p| plugin(p),
                |value| validate(&provider, value),
            );
            let accepted = result.as_ref().is_ok_and(&mut supported);
            let result = if result.is_ok() && !accepted {
                Err("provider does not support this resource".into())
            } else {
                result
            };
            results.push((provider, result));
            if exclusive || (first && accepted) {
                break;
            }
        }
        results
    }
}

pub fn registry(
    feature: &str,
    ext: &str,
    mime: &str,
    is_dir: bool,
    providers: Vec<Provider>,
) -> Vec<Provider> {
    let method = match feature {
        "info" => "info.resolve",
        "preview" => "preview.open",
        _ => feature,
    };
    EventRouter::providers(
        method,
        &[json!({"extension":ext,"mime":mime,"kind":if is_dir {"directory"} else {"file"}})],
        "",
        providers,
    )
}

/// Native RPC supplies timeouts and operation cleanup. Built-in failures are isolated
/// at the same boundary; each feature validates its own result before collection.
pub fn dispatch<T>(
    provider: &Provider,
    builtin: impl FnOnce(&str) -> Result<Value, String>,
    plugin: impl FnOnce(&crate::plugin_host::PluginInfo) -> Result<Value, String>,
    validate: impl FnOnce(Value) -> Result<T, String>,
) -> Result<T, String> {
    let value =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match &provider.handler {
            Handler::Builtin(id) => builtin(id),
            Handler::Plugin(p) => plugin(p),
        }))
        .map_err(|_| "provider panicked".to_string())??;
    if serde_json::to_vec(&value).map_err(|e| e.to_string())?.len() > 4 * 1024 * 1024 {
        return Err("provider result exceeds limit".into());
    }
    validate(value)
}

pub fn is_event(method: &str) -> bool {
    matches!(
        method,
        "info.resolve"
            | "preview.open"
            | "decorations.resolve"
            | "contextMenu.resolve"
            | "contextMenu.execute"
            | "toolbar.invoke"
            | "search.query"
            | "connection.open"
    )
}

pub fn envelope(method: &str, params: Value) -> Value {
    if !is_event(method) || params.get("options").is_some() {
        return params;
    }
    json!({"target": null, "selection": [], "options": params})
}

/// Project only Core-bound resources; no plugin-supplied reference grants access.
pub fn prepare(
    method: &str,
    params: Value,
    scope: &mut Option<crate::plugin_fs_access::Scope>,
    metadata_allowed: bool,
) -> Result<Value, String> {
    let mut request = envelope(method, params);
    if !is_event(method) {
        return Ok(request);
    }
    if let Some(options) = request.get_mut("options").and_then(Value::as_object_mut) {
        for key in [
            "targetRef",
            "selectionRefs",
            "directoryRef",
            "selectionMetadata",
        ] {
            options.remove(key);
        }
    }
    if let Some(scope) = scope {
        if method == "contextMenu.resolve" {
            scope.restrict_to_metadata();
        }
        let already_bound = request["selection"]
            .as_array()
            .filter(|items| !items.is_empty())
            .is_some_and(|items| {
                items.iter().all(|item| {
                    scope.resolve_reference(&item["ref"]).is_ok()
                        || scope
                            .metadata_reference(&item["ref"])
                            .ok()
                            .flatten()
                            .is_some()
                })
            });
        if already_bound {
            if !metadata_allowed {
                if let Some(target) = request["target"].as_object_mut() {
                    target.remove("metadata");
                }
                for item in request["selection"].as_array_mut().unwrap() {
                    if let Some(object) = item.as_object_mut() {
                        object.remove("metadata");
                    }
                }
            }
            return Ok(request);
        }
        let mut paths = scope.selected_paths();
        if paths.is_empty() && matches!(method, "decorations.resolve" | "search.query") {
            paths.push(scope.path().to_path_buf());
        }
        let mut selection = Vec::new();
        for (index, path) in paths.iter().enumerate() {
            let projected = if index == 0 && request.get("target").is_some_and(Value::is_object) {
                // A broker-prepared target reuses its metadata and identity.
                let mut target = request["target"].clone();
                if !metadata_allowed {
                    target.as_object_mut().unwrap().remove("metadata");
                }
                target
            } else if let Some(existing) = request["selection"]
                .get(index)
                .filter(|v| v.get("ref").is_some())
            {
                let mut value = existing.clone();
                if !metadata_allowed {
                    value
                        .as_object_mut()
                        .ok_or("invalid resource")?
                        .remove("metadata");
                }
                value
            } else {
                crate::plugin_resource_broker::Resource::new("", path).project(metadata_allowed)
            };
            let reference =
                serde_json::from_value(projected["ref"].clone()).map_err(|e| e.to_string())?;
            scope.bind_reference(&reference, path)?;
            selection.push(projected);
        }
        if request["target"].is_null() {
            request["target"] = selection.first().cloned().unwrap_or(Value::Null);
        }
        if !paths.is_empty() {
            request["selection"] = json!(selection);
        }
        if !metadata_allowed {
            if let Some(target) = request["target"].as_object_mut() {
                target.remove("metadata");
            }
            if let Some(selection) = request["selection"].as_array_mut() {
                for resource in selection {
                    if let Some(object) = resource.as_object_mut() {
                        object.remove("metadata");
                    }
                }
            }
        }
    }
    Ok(request)
}

/// All feature invocations share operation construction. A remote metadata-only
/// operation has an empty local scope and ref-indexed attributes, never a download.
pub fn invoke_selection(
    plugin: &str,
    method: &str,
    request: Value,
    directory: &str,
    paths: &[std::path::PathBuf],
) -> Result<Value, String> {
    if !crate::mounts::handles(directory) {
        return crate::plugin_host::global()
            .invoke_selected(plugin, method, request, directory, paths);
    }
    if method != "contextMenu.resolve" {
        return Err("remote content must be materialized by the resource broker".into());
    }
    let mut scope = crate::plugin_fs_access::Scope::selected(&std::env::temp_dir(), &[])?;
    scope.restrict_to_metadata();
    for resource in request["selection"].as_array().into_iter().flatten() {
        let mut metadata = resource["metadata"].clone();
        metadata["path"] = resource["ref"]["nodeId"].clone();
        scope.bind_metadata(&resource["ref"], metadata)?;
    }
    crate::plugin_host::global().invoke_scoped(plugin, method, request, Some(scope))
}

pub fn invoke_directory(
    plugin: &str,
    method: &str,
    request: Value,
    directory: &str,
    paths: &[std::path::PathBuf],
) -> Result<Value, String> {
    let remote = crate::mounts::handles(directory);
    let mut scope = if remote {
        crate::plugin_fs_access::Scope::selected(&std::env::temp_dir(), &[])?
    } else {
        crate::plugin_fs_access::Scope::new(Path::new(directory))?
    };
    if remote {
        scope.restrict_to_metadata();
    }
    if request["target"].get("ref").is_some() {
        if remote {
            scope.bind_metadata(
                &request["target"]["ref"],
                request["target"]["metadata"].clone(),
            )?;
        } else {
            let reference = serde_json::from_value(request["target"]["ref"].clone())
                .map_err(|e| e.to_string())?;
            scope.bind_reference(&reference, Path::new(directory))?;
        }
    }
    for (item, path) in request["selection"]
        .as_array()
        .into_iter()
        .flatten()
        .zip(paths)
    {
        if remote {
            scope.bind_metadata(&item["ref"], item["metadata"].clone())?;
        } else {
            let reference =
                serde_json::from_value(item["ref"].clone()).map_err(|e| e.to_string())?;
            scope.bind_reference(&reference, path)?;
        }
    }
    crate::plugin_host::global().invoke_scoped(plugin, method, request, Some(scope))
}

pub fn invoke(plugin: &str, method: &str, params: Value, local: &Path) -> Result<Value, String> {
    crate::plugin_host::global().invoke_selected(
        plugin,
        method,
        params,
        &local.parent().unwrap_or(Path::new(".")).to_string_lossy(),
        &[local.to_path_buf()],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn references_are_bound_to_operation_and_metadata_is_projected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("chosen.txt");
        std::fs::write(&path, b"abc").unwrap();
        let scope = crate::plugin_fs_access::Scope::selected(dir.path(), &[path.clone()]).unwrap();
        let mut first = Some(scope.clone());
        let request = prepare("contextMenu.resolve", json!({}), &mut first, false).unwrap();
        let target = &request["target"];
        assert!(target.get("metadata").is_none());
        let first = first.unwrap();
        assert!(first
            .resolve_reference(&target["ref"])
            .unwrap()
            .ends_with("chosen.txt"));
        assert!(first.authorize_method("host.fs.openRead").is_err());
        assert!(first.authorize_method("host.fs.beginWrite").is_err());
        assert!(first.authorize_method("host.process.run").is_err());
        assert!(first.authorize_method("host.fs.stat").is_ok());
        assert!(first.clone().authorize_method("host.fs.openRead").is_err());
        assert!(scope.resolve_reference(&target["ref"]).is_err());
        let mut second = Some(scope);
        let second_request = prepare("info.resolve", json!({}), &mut second, true).unwrap();
        assert_eq!(second_request["target"]["metadata"]["name"], "chosen.txt");
        assert!(second.unwrap().resolve_reference(&target["ref"]).is_err());
    }

    #[test]
    fn provider_failure_is_isolated_and_validation_runs() {
        let provider = Provider::builtin("test", 1);
        let failure = dispatch(
            &provider,
            |_| panic!("broken provider"),
            |_| unreachable!(),
            Ok::<_, String>,
        );
        assert!(failure.unwrap_err().contains("panicked"));
        assert!(dispatch(
            &provider,
            |_| Ok(json!({})),
            |_| unreachable!(),
            |_| Err::<Value, _>("bad result".into())
        )
        .is_err());
    }

    #[test]
    fn feature_options_are_not_rewrapped() {
        let p = json!({"target": null,"selection": [],"options":{"query":"hello"}});
        assert_eq!(envelope("search.query", p.clone()), p);
        assert_eq!(
            envelope("search.query", json!({"query":"hello"}))["options"]["query"],
            "hello"
        );
        assert_eq!(envelope("custom.method", json!({"x":1})), json!({"x":1}));
    }
}
