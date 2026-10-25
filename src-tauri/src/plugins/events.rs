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
/// Built-in and native providers share matching, ordering and collection.
pub fn registry(
    feature: &str,
    ext: &str,
    mime: &str,
    is_dir: bool,
    mut providers: Vec<Provider>,
) -> Vec<Provider> {
    for plugin in crate::plugin_host::catalog() {
        if !plugin.enabled || !plugin.approved || !plugin.backend_available {
            continue;
        }
        let Some(contribution) = plugin.extensions.get(feature) else {
            continue;
        };
        if !crate::plugin_match::MatchRules::from_contribution(contribution)
            .matches(ext, mime, is_dir)
        {
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
        request["selection"] = json!(selection);
    }
    Ok(request)
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
