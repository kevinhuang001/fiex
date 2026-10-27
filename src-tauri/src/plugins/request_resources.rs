//! Project request resource identities and replace legacy host-path inputs at
//! the last Core boundary. This module never grants new filesystem access.
use crate::{plugin_fs_access::Scope, plugin_resource_context::ResourceSnapshot};
use serde_json::{json, Value};
use std::{collections::HashMap, path::Path};
pub fn project(
    plugin: &str,
    params: &mut Value,
    scope: &mut Scope,
    metadata: bool,
) -> Result<(), String> {
    if !params.is_object() {
        return Ok(());
    }
    let paths = scope.selected_paths();
    if params.get("targets").is_none() {
        let mut targets = Vec::new();
        for path in &paths {
            let resource = ResourceSnapshot::new(&path.to_string_lossy(), path);
            let target = json!(resource.project(plugin, "", metadata, false));
            scope.bind_resource(&target, Some(path))?;
            targets.push(target);
        }
        params["targets"] = json!(targets);
    }
    let mut aliases = HashMap::new();
    for target in params["targets"].as_array().into_iter().flatten() {
        if let Ok(path) = scope.resolve_reference(&target["ref"]) {
            aliases.insert(path.clone(), target["ref"].clone());
            if let Some(name) = Path::new(&path).file_name().and_then(|name| name.to_str()) {
                aliases.insert(name.into(), target["ref"].clone());
            }
        }
    }
    let root = params["directory"]
        .as_str()
        .map(std::path::PathBuf::from)
        .filter(|path| scope.authorize(&path.to_string_lossy()).is_ok())
        .unwrap_or_else(|| scope.path().to_path_buf());
    if scope.authorize(&root.to_string_lossy()).is_ok() {
        let resource = ResourceSnapshot::new(&root.to_string_lossy(), &root);
        let directory = json!(resource.project(plugin, "", metadata, false));
        scope.bind_resource(&directory, Some(&root))?;
        params["directory"] = directory;
    }
    for field in ["path", "source", "destination", "root"] {
        if let Some(reference) = params[field]
            .as_str()
            .and_then(|path| aliases.get(path))
            .cloned()
        {
            params[field] = reference;
        }
    }
    if let Some(paths) = params["paths"].as_array_mut() {
        for path in paths {
            if let Some(reference) = path.as_str().and_then(|path| aliases.get(path)).cloned() {
                *path = reference;
            }
        }
    }
    Ok(())
}
