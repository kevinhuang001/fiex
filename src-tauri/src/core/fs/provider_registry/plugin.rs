//! Adapter for plugin owned connection and view mounts.
use super::{FsError, FsErrorKind, ResourceBroker};
use crate::core::volumes::native_vfs;
use crate::plugins::mounts;
use fiex_provider_contracts::{FsEntry, FsRead, FsWrite};
use std::{
    io::Read,
    sync::{Arc, Mutex},
};

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

pub(super) fn stream_read(resource: &ResourceBroker) -> Result<FsRead, FsError> {
    let (mount, node) = plugin_node(resource)?;
    let stream = mount.open_read(&node).map_err(FsError::from)?;
    let size = stream
        .size()
        .or_else(|| mount.stat(&node).ok().and_then(|entry| entry.size))
        .ok_or_else(|| FsError::new(FsErrorKind::Unsupported, "stream size is unknown"))?;
    Ok(FsRead::new(PluginReader { stream, offset: 0 }, Some(size)))
}

struct PluginReader {
    stream: crate::plugins::providers::fs::FsStream,
    offset: u64,
}

impl Read for PluginReader {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let (bytes, _) = self
            .stream
            .read_at(self.offset, output.len().min(super::super::CHUNK))
            .map_err(std::io::Error::other)?;
        output[..bytes.len()].copy_from_slice(&bytes);
        self.offset = self
            .offset
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| std::io::Error::other("read offset overflow"))?;
        Ok(bytes.len())
    }
}

pub(super) fn stream_write(
    resource: &ResourceBroker,
    create: bool,
    size: Option<u64>,
) -> Result<FsWrite, FsError> {
    let stream = if create {
        let (mount, parent, name) = parent(resource)?;
        mount
            .begin_write(crate::plugins::providers::fs::WriteRequest {
                node_id: None,
                parent_id: Some(&parent),
                name: Some(&name),
                size,
                if_revision: None,
            })
            .map_err(FsError::from)?
    } else {
        let (mount, node) = plugin_node(resource)?;
        let entry = mount.stat(&node).map_err(FsError::from)?;
        mount
            .begin_write(crate::plugins::providers::fs::WriteRequest {
                node_id: Some(&node),
                parent_id: None,
                name: None,
                size,
                if_revision: entry.revision.as_deref(),
            })
            .map_err(FsError::from)?
    };
    let stream = Arc::new(Mutex::new(Some(stream)));
    let writer = PluginWriter(Arc::clone(&stream));
    let finish = Arc::clone(&stream);
    let path = resource.path.clone();
    Ok(FsWrite::with_commit(writer, move || {
        let entry = finish
            .lock()
            .map_err(|_| {
                fiex_provider_contracts::ProviderError::internal("write stream unavailable")
            })?
            .take()
            .ok_or_else(|| fiex_provider_contracts::ProviderError::internal("write stream closed"))?
            .commit()
            .map_err(|error| {
                fiex_provider_contracts::ProviderError::internal(FsError::from(error).to_string())
            })?;
        if let Some(entry) = entry {
            remember_entry(&path, &entry).map_err(|error| {
                fiex_provider_contracts::ProviderError::internal(error.to_string())
            })?;
            Ok(entry)
        } else {
            Ok(FsEntry::default())
        }
    }))
}

struct PluginWriter(Arc<Mutex<Option<crate::plugins::providers::fs::FsWriteStream>>>);

impl std::io::Write for PluginWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let mut stream = self
            .0
            .lock()
            .map_err(|_| std::io::Error::other("write stream unavailable"))?;
        let stream = stream
            .as_mut()
            .ok_or_else(|| std::io::Error::other("write stream closed"))?;
        stream.write(bytes).map_err(std::io::Error::other)?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
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
