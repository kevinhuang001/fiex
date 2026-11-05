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
                .map(
                    |path| crate::plugins::resource::resource_context::new_resource_snapshot(
                        &path.to_string_lossy(),
                        path
                    )
                    .project(plugin, "", true, false)
                )
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
    if let Some(folder) = directory.filter(|value| {
        crate::core::fs::provider_registry::resource_access(value).is_ok_and(|access| {
            access == crate::core::fs::provider_registry::ResourceAccess::HostPath
        })
    }) {
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
    let access = resources
        .iter()
        .map(|resource| {
            crate::core::fs::provider_registry::resource_access(&resource.display)
                .map_err(String::from)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let directory_access = directory
        .map(crate::core::fs::provider_registry::resource_access)
        .transpose()
        .map_err(String::from)?;
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
    // The trusted target count is separate from the metadata projection so
    // blank-canvas actions and multi-selection policies keep exact semantics.
    params["selectionCount"] = json!(resources.len());
    if let Some(directory) = directory {
        let snapshot = crate::plugins::resource::resource_context::resolve(directory)?;
        if !snapshot.is_dir {
            return Err("event directory is not a directory".into());
        }
        params["directory"] = json!(snapshot.project(
            &plugin.id,
            "",
            policy.allowed(crate::plugins::security::policy::Capability::Metadata),
            false,
        ));
    }
    let mut paths = resources
        .iter()
        .zip(&access)
        .filter(|(_, access)| {
            **access == crate::core::fs::provider_registry::ResourceAccess::HostPath
        })
        .map(|(resource, _)| resource)
        .map(|r| crate::plugins::resource::fs_access::canonical_selection(&r.local))
        .collect::<Result<Vec<_>, _>>()?;
    let host_directory = (directory_access
        == Some(crate::core::fs::provider_registry::ResourceAccess::HostPath))
    .then_some(directory)
    .flatten();
    for folder in action_folders(method, host_directory, &paths, policy.declared())? {
        if !paths.contains(&folder) {
            paths.push(folder);
        }
    }
    for (resource, target) in resources
        .iter()
        .zip(params["targets"].as_array_mut().unwrap())
    {
        if policy.allowed(crate::plugins::security::policy::Capability::Metadata)
            && target["metadata"].is_object()
        {
            let caps = resource.operations.unwrap_or_else(|| {
                crate::plugins::resource::file_capabilities::for_path(&resource.display)
            });
            target["metadata"]["operations"] = json!({"stat":caps.stat,"list":caps.list,"search":caps.search,"read":caps.read,"write":caps.write,"mkdir":caps.create,"rename":caps.rename,"remove":caps.remove});
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
    for ((resource, target), access) in resources
        .iter()
        .zip(params["targets"].as_array().unwrap())
        .zip(&access)
    {
        let local = if *access == crate::core::fs::provider_registry::ResourceAccess::HostPath {
            Some(crate::plugins::resource::fs_access::canonical_selection(
                &resource.local,
            )?)
        } else {
            None
        };
        scope.bind_resource(target, local.as_deref())?;
        if (content || directory_event)
            && *access == crate::core::fs::provider_registry::ResourceAccess::Provider
        {
            scope.bind_remote(target, resource)?;
        }
    }
    // Directory events explicitly authorize their requested root, not an unrelated current pane.
    if matches!(method, "decorations.resolve" | "search.query") {
        if let Some(dir) = host_directory {
            scope = crate::plugins::resource::fs_access::Scope::new(Path::new(dir))?;
            for ((resource, target), access) in resources
                .iter()
                .zip(params["targets"].as_array().unwrap())
                .zip(&access)
            {
                let local = (*access
                    == crate::core::fs::provider_registry::ResourceAccess::HostPath)
                    .then_some(resource.local.as_path());
                scope.bind_resource(target, local)?;
                if *access == crate::core::fs::provider_registry::ResourceAccess::Provider {
                    scope.bind_remote(target, resource)?;
                }
            }
        }
    }
    super::directory_scope::bind(
        &mut params,
        &mut scope,
        &policy,
        method,
        resources,
        directory,
    )?;
    let read_only_vfs = resources
        .iter()
        .any(|resource| read_only_vfs(&resource.display, resource.operations))
        || (resources.is_empty() && directory.is_some_and(|path| read_only_vfs(path, None)));
    if read_only_vfs {
        scope.restrict_to_readonly();
    }
    if matches!(
        method,
        "hooks.invoke" | "contextMenu.execute" | "toolbar.invoke" | "preview.open"
    ) {
        scope.set_user_action_context(
            crate::core::fs::mapped_access::UserActionContext::from_core_event(),
        );
    }
    Ok((params, scope))
}

/// A VFS view can restrict general Host APIs such as process execution in
/// addition to the per-operation checks enforced by ResourceBroker. Derive
/// that policy from the trusted view and its effective capability snapshot,
/// never from a special URI or provider name.
fn read_only_vfs(path: &str, capabilities: Option<fiex_resource::FileCapabilities>) -> bool {
    let (view, _) = crate::plugins::match_rules::typed_locations_for(path);
    if view
        .is_none_or(|location| location.resource_type != fiex_provider_contracts::ResourceType::Vfs)
    {
        return false;
    }
    !capabilities
        .unwrap_or_else(|| crate::plugins::resource::file_capabilities::for_path(path))
        .has_mutation_capability()
}
