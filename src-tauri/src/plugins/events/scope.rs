use super::*;
use std::path::{Path, PathBuf};
/// One shared request preparation path, at the host boundary. No business
/// module may grant privileges by supplying a reference or a read token.
pub fn prepare(
    plugin: &str,
    method: &str,
    mut params: Value,
    scope: &mut Option<crate::plugins::resource::fs_access::Scope>,
    declared: &std::collections::BTreeSet<String>,
) -> Result<Value, String> {
    if event(method).is_none() {
        return Ok(params);
    }
    if !params.is_object() {
        return Err("event parameters must be an object".into());
    }
    let policy = crate::plugins::security::policy::PermissionPolicy::new(plugin, declared);
    let metadata_allowed = policy.allowed(crate::plugins::security::policy::Capability::Metadata);
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

/// A user-initiated open hook or context-menu action may need to create its
/// result next to the selection ("extract here", "compress here"). Only those
/// explicit events get the containing folder, and only when the manifest
/// declares a create capability; every write still passes the capability
/// check in host dispatch, so a read-only plugin gains nothing.
pub(super) fn action_folders(
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
    if let Some(folder) = directory.filter(|value| !crate::plugins::mounts::handles(value) && !crate::core::volumes::virtual_resolvers::opaque(value)) {
        folders.push(crate::plugins::resource::fs_access::canonical_selection(
            Path::new(folder),
        )?);
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
    plugin: &crate::plugins::host::PluginInfo,
    method: &str,
    mut params: Value,
    resources: &[ResourceSnapshot],
    directory: Option<&str>,
) -> Result<(Value, crate::plugins::resource::fs_access::Scope), String> {
    require_event(method)?;
    if !matching::matches_resources(method, &plugin.extensions, resources, directory) {
        return Err("provider does not match event targets".into());
    }
    let declared = plugin.declared_permissions.iter().cloned().collect();
    let policy = crate::plugins::security::policy::PermissionPolicy::new(&plugin.id, &declared);
    let content = !matches!(
        method,
        "contextMenu.resolve" | "decorations.resolve" | "search.query"
    ) && (policy.allowed(crate::plugins::security::policy::Capability::FilesRead)
        || method == "contextMenu.execute"
        || method == "toolbar.invoke");
    params["targets"] = json!(resources
        .iter()
        .map(|r| r.project(
            &plugin.id,
            "",
            policy.allowed(crate::plugins::security::policy::Capability::Metadata),
            false
        ))
        .collect::<Vec<_>>());
    let mut paths = resources
        .iter()
        .filter(|r| !crate::plugins::mounts::handles(&r.display) && (!crate::core::volumes::virtual_resolvers::opaque(&r.display) || crate::core::volumes::applications::is_user(&r.display)))
        .map(|r| crate::plugins::resource::fs_access::canonical_selection(&r.local))
        .collect::<Result<Vec<_>, _>>()?;
    for folder in action_folders(method, directory, &paths, policy.declared())? {
        if !paths.contains(&folder) {
            paths.push(folder);
        }
    }
    if policy.allowed(crate::plugins::security::policy::Capability::Metadata) {
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
    params["location"] = json!({
        "kind": if directory.is_some_and(crate::core::volumes::virtual_resolvers::opaque) { "vfs" }
                else if directory.is_some_and(crate::plugins::mounts::handles) { "vfs" } else { "fs" },
        "providerId": directory.and_then(|d| crate::core::volumes::virtual_resolvers::resolver_for(d).map(|p| p.id().to_owned())
            .or_else(|| crate::plugins::resolvers::fs::mount_plugin_id(d).ok())),
        "selectionCount": resources.len(),
    });
    for (resource, target) in resources.iter().zip(params["targets"].as_array_mut().unwrap()) {
        if policy.allowed(crate::plugins::security::policy::Capability::Metadata) && target["metadata"].is_object() {
            let mut caps = resource.operations.unwrap_or_else(|| crate::plugins::resource::file_capabilities::for_path(&resource.display));
            if directory == Some("recent://") { caps = caps.intersect(crate::plugins::resource::file_capabilities::FileCapabilities::READ_ONLY); }
            target["metadata"]["operations"] = json!({"read":caps.read,"write":caps.write,"mkdir":caps.create,"rename":caps.rename,"remove":caps.remove});
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
        crate::plugins::resource::fs_access::Scope::selected(&root, &paths)?
    } else {
        crate::plugins::resource::fs_access::Scope::selected(&std::env::temp_dir(), &[])?
    };
    let directory_event = matches!(method, "decorations.resolve" | "search.query");
    if method == "contextMenu.resolve" || (!content && !directory_event) {
        scope.restrict_to_metadata();
    }
    for (resource, target) in resources.iter().zip(params["targets"].as_array().unwrap()) {
        let local = if !crate::plugins::mounts::handles(&resource.display) && (!crate::core::volumes::virtual_resolvers::opaque(&resource.display) || crate::core::volumes::applications::is_user(&resource.display)) {
            Some(crate::plugins::resource::fs_access::canonical_selection(
                &resource.local,
            )?)
        } else {
            None
        };
        scope.bind_resource(target, local.as_deref())?;
        if (content || directory_event) && crate::plugins::mounts::handles(&resource.display) {
            scope.bind_remote(target, resource)?;
        }
    }
    // Directory events explicitly authorize their requested root, not an unrelated current pane.
    if matches!(method, "decorations.resolve" | "search.query") {
        if let Some(dir) = directory.filter(|d| !crate::plugins::mounts::handles(d) && !crate::core::volumes::virtual_resolvers::opaque(d)) {
            scope = crate::plugins::resource::fs_access::Scope::new(Path::new(dir))?;
            for (r, target) in resources.iter().zip(params["targets"].as_array().unwrap()) {
                scope.bind_resource(target, Some(&r.local))?;
            }
        }
    }
    if directory == Some("recent://") { scope.restrict_to_readonly(); }
    if directory.and_then(crate::core::volumes::virtual_resolvers::resolver_for).is_some_and(|p| p.id() == "core:trash") { scope.restrict_to_metadata(); }
    Ok((params, scope))
}
