//! Adapter for plugin owned connection and view mounts.
use super::{FsError, ResourceBroker};
use crate::core::volumes::native_vfs;
use crate::plugins::mounts;
use fiex_provider_contracts::FsEntry;

mod streams;
pub(super) use streams::{stream_read, stream_write};

pub(super) fn validate_child(parent: &str, child: &str) -> Result<(), String> {
    let root = mounts::parse(parent)?;
    let candidate = mounts::parse(child)?;
    if root.mount_id != candidate.mount_id
        || !std::path::Path::new(&candidate.relative).starts_with(&root.relative)
    {
        return Err("child outside selected resource".into());
    }
    Ok(())
}

pub(super) fn child_path(parent: &str, name: &str) -> Result<String, String> {
    let uri = mounts::parse(parent)?;
    let relative = if uri.relative.is_empty() {
        name.to_owned()
    } else {
        format!("{}/{name}", uri.relative.trim_end_matches('/'))
    };
    mounts::uri(&uri.mount_id, &relative)
}

pub(super) fn parent_path(path: &str) -> Result<String, String> {
    let uri = mounts::parse(path)?;
    if uri.relative.is_empty() {
        return Err("mount root has no parent".into());
    }
    let parent = uri
        .relative
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .unwrap_or("");
    mounts::uri(&uri.mount_id, parent)
}

pub(super) fn plugin_node(
    resource: &ResourceBroker,
) -> Result<(crate::plugins::providers::fs::Mount, String), FsError> {
    if crate::remote::is_remote(&resource.path) {
        crate::remote::provider_node(&resource.path).map_err(FsError::from)
    } else {
        crate::plugins::vfs::provider_node(&resource.path).map_err(FsError::from)
    }
}

pub(super) fn parent(
    resource: &ResourceBroker,
) -> Result<(crate::plugins::providers::fs::Mount, String, String), FsError> {
    let path = super::parent_path(&resource.path)?;
    let reference = resource.scope.reference_for_path(&path)?;
    let parent = ResourceBroker::resolve(&resource.scope, &resource.owner, &reference)?;
    let (mount, node) = plugin_node(&parent)?;
    let name = mounts::parse(&resource.path)?
        .relative
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .ok_or("InvalidVfsUri: resource has no child name")?
        .to_owned();
    mounts::segment(&name)?;
    Ok((mount, node, name))
}

pub(super) fn remember_entry(path: &str, entry: &FsEntry) -> Result<(), FsError> {
    if crate::remote::is_remote(path) {
        crate::remote::record_provider_mutation(path, entry).map_err(FsError::from)
    } else {
        let uri = mounts::parse(path)?;
        mounts::bind_node(&uri.mount_id, &uri.relative, &entry.node_id).map_err(FsError::from)
    }
}

pub(super) fn project_entry(
    mount: &crate::plugins::providers::fs::Mount,
    path: &str,
    entry: FsEntry,
) -> crate::core::files::Entry {
    let mut core = native_vfs::core_entry(entry.clone());
    core.path = path.to_owned();
    core.allowed_ops = fiex_provider_contracts::FsOperation::ALL
        .into_iter()
        .filter(|operation| mount.supports(*operation) && entry.allows(*operation))
        .map(|operation| operation.as_str().to_owned())
        .collect();
    core
}

fn project_cache_state(entry: &mut crate::core::files::Entry, state: &crate::remote::CacheState) {
    entry.has_cache = state.has_cache;
    entry.pinned = state.pinned;
    entry.uploaded = state.uploaded;
    entry.sync = Some(
        if state.conflict {
            "conflict"
        } else if state.dirty {
            "pending"
        } else if state.pinned {
            "pinned"
        } else if state.cached {
            "synced"
        } else if state.partial {
            "partial"
        } else {
            "cloud"
        }
        .into(),
    );
}

fn child_plugin_path(parent: &str, entry: &FsEntry) -> Result<String, FsError> {
    mounts::segment(&entry.name)?;
    let uri = mounts::parse(parent)?;
    let relative = if uri.relative.is_empty() {
        entry.name.clone()
    } else {
        format!("{}/{}", uri.relative.trim_end_matches('/'), entry.name)
    };
    mounts::uri(&uri.mount_id, &relative).map_err(FsError::from)
}

pub(super) fn stat(resource: &ResourceBroker) -> Result<crate::core::files::Entry, FsError> {
    if crate::remote::is_remote(&resource.path) {
        return crate::remote::stat(&resource.path).map_err(FsError::from);
    }
    let (mount, node) = plugin_node(resource)?;
    let entry = mount.stat(&node).map_err(FsError::from)?;
    Ok(project_entry(&mount, &resource.path, entry))
}

pub(super) fn list(
    resource: &ResourceBroker,
) -> Result<(Vec<crate::core::files::Entry>, Vec<String>), FsError> {
    let (mount, node) = plugin_node(resource)?;
    let remote_parent = crate::remote::is_remote(&resource.path);
    let listing = match mount.list(&node, true, &[]) {
        Ok(listing) => listing,
        Err(error) if remote_parent => {
            if let Some((entries, warnings)) =
                crate::remote::offline_provider_listing(&resource.path, &error)
                    .map_err(FsError::from)?
            {
                return Ok((entries, warnings));
            }
            return Err(FsError::from(error));
        }
        Err(error) => return Err(FsError::from(error)),
    };
    let complete = !listing.truncated;
    let mut entries = Vec::with_capacity(listing.entries.len());
    let mut remote_entries = Vec::new();
    for entry in listing.entries {
        let path = child_plugin_path(&resource.path, &entry)?;
        if remote_parent {
            remote_entries.push((path.clone(), entry.clone()));
        } else {
            remember_entry(&path, &entry)?;
        }
        entries.push(project_entry(&mount, &path, entry));
    }
    let cache_states = if remote_parent {
        crate::remote::observe_provider_listing(&resource.path, &remote_entries, complete)
            .map_err(FsError::from)?
    } else {
        std::collections::HashMap::new()
    };
    for entry in &mut entries {
        if let Some(state) = cache_states.get(&entry.path) {
            project_cache_state(entry, state);
        }
    }
    Ok((entries, Vec::new()))
}

pub(super) fn walk_search(
    resource: &ResourceBroker,
    query: &str,
    hidden: bool,
    ignores: &[String],
    limit: usize,
    cancelled: &dyn Fn() -> bool,
    emit: &mut dyn FnMut(crate::core::files::Entry),
) -> Result<(bool, usize), FsError> {
    let mut pending = vec![resource.path.clone()];
    let mut visited = std::collections::HashSet::new();
    let mut count = 0usize;
    let mut truncated = false;
    let filter = crate::core::ignores::IgnoreSet::new(ignores);
    while let Some(path) = pending.pop() {
        if cancelled() {
            break;
        }
        if !visited.insert(path.clone()) {
            continue;
        }
        if visited.len() > 1000 {
            truncated = true;
            break;
        }
        let reference = resource.scope.reference_for_path(&path)?;
        let directory = ResourceBroker::resolve(&resource.scope, &resource.owner, &reference)?;
        super::require(
            &directory,
            fiex_provider_contracts::FsOperation::List,
            false,
        )?;
        let (entries, _) = list(&directory)?;
        for entry in entries {
            if cancelled() {
                break;
            }
            if (!hidden && entry.name.starts_with('.'))
                || filter.is_ignored(std::path::Path::new(&entry.name))
            {
                continue;
            }
            if entry.is_dir && !entry.is_symlink {
                pending.push(entry.path.clone());
            }
            if entry.name.to_lowercase().contains(&query.to_lowercase()) {
                if count == limit {
                    truncated = true;
                    break;
                }
                count += 1;
                emit(entry);
            }
        }
        if truncated {
            break;
        }
    }
    Ok((truncated || !pending.is_empty(), 0))
}
