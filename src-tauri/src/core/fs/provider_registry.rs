//! Core-owned routing into shared VFS providers.
//!
//! Native providers are invoked through the shared contract registry. Plugin
//! mounts use the same typed contract through their existing IPC proxy.
use super::{
    handles::{ProviderReadHandle, ProviderWriteHandle},
    FsError, FsErrorKind, ReadHandle, ResourceBroker, WriteHandle,
};
use crate::{core::volumes::native_vfs, plugins::mounts};
use fiex_provider_contracts::{FsEntry, FsRead, FsWrite, TransferPreflight};
use serde_json::{to_value, Value};
use std::{io::Read, sync::{Arc, Mutex}};

enum Route {
    Plugin,
    NativeProvider,
}

fn unregistered_scheme(path: &str) -> bool {
    let Some(index) = path.find(':') else {
        return false;
    };
    let prefix = &path[..index];
    if prefix.len() == 1 && prefix.as_bytes()[0].is_ascii_alphabetic() {
        return false;
    }
    !prefix.is_empty()
        && prefix.as_bytes()[0].is_ascii_alphabetic()
        && prefix
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
}

fn resolve(path: &str) -> Result<Route, FsError> {
    if mounts::handles(path) {
        return Ok(Route::Plugin);
    }
    if native_vfs::shared_provider_id(path).is_some() {
        return Ok(Route::NativeProvider);
    }
    if path.is_empty() || path.contains("://") || unregistered_scheme(path) {
        return Err("Unsupported: unregistered filesystem URI".into());
    }
    Err("Unsupported: unregistered filesystem resource".into())
}

/// The same Core-owned storage identity drives operations and resource matching.
pub(crate) fn identity(path: &str) -> Option<(&'static str, String, String)> {
    match resolve(path).ok()? {
        Route::Plugin => {
            let (uri, source) = mounts::resolve(path).ok()?;
            let provider = match source {
                mounts::Source::Connection(connection) => {
                    format!("plugin:{}:{}", connection.plugin_id, connection.kind)
                }
                mounts::Source::View(view) => format!(
                    "plugin:{}:{}",
                    view.descriptor.plugin_id, view.descriptor.contribution_id
                ),
            };
            Some(("vfs", provider, uri.mount_id))
        }
        Route::NativeProvider => {
            let provider = native_vfs::shared_provider_id(path)?;
            if provider == "core:local-fs" {
                return Some(("fs", provider.into(), "local".into()));
            }
            if provider == "core:usb-fs" {
                let route = crate::usbfs::route(path)?;
                let generation = crate::usbfs::generation(&route).unwrap_or_else(|_| "offline".into());
                return Some(("fs", provider.into(), format!("core:usb:{generation}")));
            }
            if !matches!(native_vfs::storage_projection(path), native_vfs::NativeStorageProjection::Owned) {
                return None;
            }
            Some(("vfs", provider.into(), provider.into()))
        }
    }
}

fn plugin_node(
    resource: &ResourceBroker,
) -> Result<(crate::plugins::resolvers::fs::Mount, String), FsError> {
    if crate::remote::is_remote(&resource.path) {
        crate::remote::provider_node(&resource.path).map_err(FsError::from)
    } else {
        crate::plugins::vfs::provider_node(&resource.path).map_err(FsError::from)
    }
}

fn plugin_parent(
    resource: &ResourceBroker,
) -> Result<(crate::plugins::resolvers::fs::Mount, String, String), FsError> {
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

fn remember_plugin_entry(path: &str, entry: &FsEntry) -> Result<(), FsError> {
    if crate::remote::is_remote(path) {
        crate::remote::remember_provider_entry(path, entry).map_err(FsError::from)
    } else {
        let uri = mounts::parse(path)?;
        mounts::bind_node(&uri.mount_id, &uri.relative, &entry.node_id).map_err(FsError::from)
    }
}

fn project_plugin_entry(
    mount: &crate::plugins::resolvers::fs::Mount,
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
    core.readonly = entry.readonly.unwrap_or(false);
    core
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

fn plugin_stat(resource: &ResourceBroker) -> Result<crate::core::files::Entry, FsError> {
    let (mount, node) = plugin_node(resource)?;
    let entry = mount.stat(&node).map_err(FsError::from)?;
    Ok(project_plugin_entry(&mount, &resource.path, entry))
}

fn plugin_list(
    resource: &ResourceBroker,
) -> Result<Vec<crate::core::files::Entry>, FsError> {
    let (mount, node) = plugin_node(resource)?;
    let listing = mount.list(&node, true, &[]).map_err(FsError::from)?;
    listing
        .entries
        .into_iter()
        .map(|entry| {
            let path = child_plugin_path(&resource.path, &entry)?;
            remember_plugin_entry(&path, &entry)?;
            Ok(project_plugin_entry(&mount, &path, entry))
        })
        .collect()
}

fn plugin_stream_read(resource: &ResourceBroker) -> Result<FsRead, FsError> {
    let (mount, node) = plugin_node(resource)?;
    let stream = mount.open_read(&node).map_err(FsError::from)?;
    let size = stream
        .size()
        .or_else(|| mount.stat(&node).ok().and_then(|entry| entry.size))
        .ok_or_else(|| FsError::new(FsErrorKind::Unsupported, "stream size is unknown"))?;
    Ok(FsRead::new(PluginReader { stream, offset: 0 }, Some(size)))
}

struct PluginReader {
    stream: crate::plugins::resolvers::fs::FsStream,
    offset: u64,
}

impl Read for PluginReader {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let (bytes, _) = self
            .stream
            .read_at(self.offset, output.len().min(super::CHUNK))
            .map_err(std::io::Error::other)?;
        output[..bytes.len()].copy_from_slice(&bytes);
        self.offset = self
            .offset
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| std::io::Error::other("read offset overflow"))?;
        Ok(bytes.len())
    }
}

fn plugin_stream_write(
    resource: &ResourceBroker,
    create: bool,
    size: Option<u64>,
) -> Result<FsWrite, FsError> {
    let stream = if create {
        let (mount, parent, name) = plugin_parent(resource)?;
        mount
            .begin_write(crate::plugins::resolvers::fs::WriteRequest {
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
            .begin_write(crate::plugins::resolvers::fs::WriteRequest {
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
            .map_err(|_| fiex_provider_contracts::ProviderError::internal("write stream unavailable"))?
            .take()
            .ok_or_else(|| fiex_provider_contracts::ProviderError::internal("write stream closed"))?
            .commit()
            .map_err(|error| fiex_provider_contracts::ProviderError::internal(FsError::from(error).to_string()))?;
        if let Some(entry) = entry {
            remember_plugin_entry(&path, &entry).map_err(|error| {
                fiex_provider_contracts::ProviderError::internal(error.to_string())
            })?;
            Ok(entry)
        } else {
            Ok(FsEntry::default())
        }
    }))
}

struct PluginWriter(Arc<Mutex<Option<crate::plugins::resolvers::fs::FsWriteStream>>>);

impl std::io::Write for PluginWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let mut stream = self.0.lock().map_err(|_| std::io::Error::other("write stream unavailable"))?;
        let stream = stream.as_mut().ok_or_else(|| std::io::Error::other("write stream closed"))?;
        stream.write(bytes).map_err(std::io::Error::other)?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub(super) fn require(
    resource: &ResourceBroker,
    operation: crate::plugins::provider_registry::FsOperation,
    create: bool,
) -> Result<(), FsError> {
    resource.ensure_current_binding()?;
    match resolve(&resource.path)? {
        Route::NativeProvider => native_vfs::require_for(resource, operation, create).map_err(FsError::from),
        Route::Plugin => {
            let (mount, node) = if create {
                let (mount, node, _) = plugin_parent(resource)?;
                (mount, node)
            } else {
                plugin_node(resource)?
            };
            let required = if create && matches!(operation, crate::plugins::provider_registry::FsOperation::Write | crate::plugins::provider_registry::FsOperation::Mkdir) {
                crate::plugins::provider_registry::FsOperation::Mkdir
            } else if operation == crate::plugins::provider_registry::FsOperation::Search
                && !mount.supports(crate::plugins::provider_registry::FsOperation::Search)
            {
                crate::plugins::provider_registry::FsOperation::List
            } else {
                operation
            };
            if !mount.supports(required) {
                return Err(FsError::new(FsErrorKind::Unsupported, format!("Unsupported: {}", required.as_str())));
            }
            let entry = mount.stat(&node).map_err(FsError::from)?;
            if !entry.allows(required) {
                return Err(FsError::new(FsErrorKind::Unsupported, format!("Unsupported: node does not permit {}", required.as_str())));
            }
            Ok(())
        }
    }
}

pub(super) fn stat(resource: &ResourceBroker) -> Result<Value, FsError> {
    let entry = match resolve(&resource.path)? {
        Route::NativeProvider => native_vfs::stat_for(resource).map_err(FsError::from)?,
        Route::Plugin => plugin_stat(resource)?,
    };
    let mut value = to_value(entry).map_err(|error| FsError::internal(error.to_string()))?;
    if let Some(symlink) = value.get("isSymlink").cloned() {
        value["symlink"] = symlink;
    }
    Ok(value)
}

pub(super) fn list(resource: &ResourceBroker) -> Result<Value, FsError> {
    let entries = match resolve(&resource.path)? {
        Route::NativeProvider => native_vfs::list_for(resource).map_err(FsError::from)?,
        Route::Plugin => plugin_list(resource)?,
    };
    to_value(entries).map_err(|error| FsError::internal(error.to_string()))
}

pub(super) fn open_read(resource: &ResourceBroker) -> Result<Box<dyn ReadHandle>, FsError> {
    require(resource, crate::plugins::provider_registry::FsOperation::Read, false)?;
    let read = match resolve(&resource.path)? {
        Route::NativeProvider => native_vfs::read_for(resource).map_err(FsError::from)?,
        Route::Plugin => plugin_stream_read(resource)?,
    };
    Ok(Box::new(ProviderReadHandle::new(read)?))
}

pub(super) fn write_stream(
    resource: &ResourceBroker,
    create: bool,
    size: Option<u64>,
) -> Result<FsWrite, FsError> {
    require(resource, crate::plugins::provider_registry::FsOperation::Write, create)?;
    match resolve(&resource.path)? {
        Route::NativeProvider => native_vfs::write_sized_for(resource, create, size).map_err(FsError::from),
        Route::Plugin => plugin_stream_write(resource, create, size),
    }
}

pub(super) fn begin_write(
    resource: &ResourceBroker,
    create: bool,
) -> Result<Box<dyn WriteHandle>, FsError> {
    require(resource, crate::plugins::provider_registry::FsOperation::Write, create)?;
    Ok(Box::new(super::handles::StagedWriteHandle::new(resource, create)?))
}

pub(super) fn begin_write_sized(
    resource: &ResourceBroker,
    create: bool,
    size: Option<u64>,
) -> Result<Box<dyn WriteHandle>, FsError> {
    let stream = write_stream(resource, create, size)?;
    Ok(Box::new(ProviderWriteHandle::new(stream)))
}

pub(super) fn mkdir(resource: &ResourceBroker) -> Result<Value, FsError> {
    require(resource, crate::plugins::provider_registry::FsOperation::Mkdir, true)?;
    let entry = match resolve(&resource.path)? {
        Route::NativeProvider => native_vfs::mkdir_for(resource)
            .map(native_vfs::core_entry)
            .map_err(FsError::from)?,
        Route::Plugin => {
            let (mount, parent, name) = plugin_parent(resource)?;
            let entry = mount.mkdir(&parent, &name).map_err(FsError::from)?;
            remember_plugin_entry(&resource.path, &entry)?;
            project_plugin_entry(&mount, &resource.path, entry)
        }
    };
    to_value(entry).map_err(|error| FsError::internal(error.to_string()))
}

pub(super) fn remove(resource: &ResourceBroker) -> Result<(), FsError> {
    require(resource, crate::plugins::provider_registry::FsOperation::Remove, false)?;
    match resolve(&resource.path)? {
        Route::NativeProvider => native_vfs::remove_for(resource).map_err(FsError::from),
        Route::Plugin => {
            let (mount, node) = plugin_node(resource)?;
            mount.remove(&node).map_err(FsError::from)
        }
    }
}

pub(super) fn rename(source: &ResourceBroker, destination: &ResourceBroker) -> Result<(), FsError> {
    require(source, crate::plugins::provider_registry::FsOperation::Rename, false)?;
    require(destination, crate::plugins::provider_registry::FsOperation::Write, true)?;
    if source.reference.mount_id != destination.reference.mount_id {
        return Err("Unsupported: cross-mount rename requires TransferEngine".into());
    }
    match resolve(&source.path)? {
        Route::NativeProvider => native_vfs::rename_for(source, destination).map(|_| ()).map_err(FsError::from),
        Route::Plugin => {
            let (mount, node) = plugin_node(source)?;
            let (destination_mount, parent, name) = plugin_parent(destination)?;
            if mount.mount_id != destination_mount.mount_id {
                return Err("Unsupported: cross-mount rename requires TransferEngine".into());
            }
            let entry = mount.rename(&node, &name, Some(&parent)).map_err(FsError::from)?;
            remember_plugin_entry(&destination.path, &entry)
        }
    }
}

pub(super) fn preflight(
    resource: &ResourceBroker,
    request: &TransferPreflight,
) -> Result<(), FsError> {
    match resolve(&resource.path)? {
        Route::NativeProvider => native_vfs::preflight_for(resource, request).map_err(FsError::from),
        Route::Plugin => {
            let (mount, node) = plugin_node(resource)?;
            let mut request = request.clone();
            request.node_id = node;
            mount.preflight(&request).map_err(FsError::from)
        }
    }
}

pub(super) fn search(
    resource: &ResourceBroker,
    query: &str,
    hidden: bool,
    ignores: &[String],
    limit: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<crate::core::files::Entry>, bool, usize), FsError> {
    let mut entries = Vec::new();
    let (truncated, skipped) = search_stream(resource, query, hidden, ignores, limit, cancelled, &mut |entry| entries.push(entry))?;
    Ok((entries, truncated, skipped))
}

pub(super) fn search_stream(
    resource: &ResourceBroker,
    query: &str,
    hidden: bool,
    ignores: &[String],
    limit: usize,
    cancelled: &dyn Fn() -> bool,
    emit: &mut dyn FnMut(crate::core::files::Entry),
) -> Result<(bool, usize), FsError> {
    require(resource, crate::plugins::provider_registry::FsOperation::Search, false)?;
    match resolve(&resource.path)? {
        Route::NativeProvider => {
            let (entries, truncated) = native_vfs::search_for(resource, query, hidden, ignores, limit, cancelled).map_err(FsError::from)?;
            for entry in entries {
                if cancelled() { break; }
                emit(entry);
            }
            Ok((truncated, 0))
        }
        Route::Plugin if !crate::remote::is_remote(&resource.path) => {
            let (mount, _) = plugin_node(resource)?;
            if mount.supports(crate::plugins::provider_registry::FsOperation::Search) {
                let truncated = crate::plugins::vfs::search_stream(
                    &resource.path, query, hidden, ignores, limit, cancelled, emit,
                ).map_err(FsError::from)?;
                return Ok((truncated, 0));
            }
            plugin_walk_search(resource, query, hidden, ignores, limit, cancelled, emit)
        }
        Route::Plugin => plugin_walk_search(resource, query, hidden, ignores, limit, cancelled, emit),
    }
}

fn plugin_walk_search(
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
        if cancelled() { break; }
        if !visited.insert(path.clone()) { continue; }
        if visited.len() > 1000 { truncated = true; break; }
        let reference = resource.scope.reference_for_path(&path)?;
        let directory = ResourceBroker::resolve(&resource.scope, &resource.owner, &reference)?;
        require(&directory, crate::plugins::provider_registry::FsOperation::List, false)?;
        let entries = plugin_list(&directory)?;
        for entry in entries {
            if cancelled() { break; }
            if (!hidden && entry.name.starts_with('.')) || filter.is_ignored(std::path::Path::new(&entry.name)) { continue; }
            if entry.is_dir && !entry.is_symlink { pending.push(entry.path.clone()); }
            if entry.name.to_lowercase().contains(&query.to_lowercase()) {
                if count == limit { truncated = true; break; }
                count += 1;
                emit(entry);
            }
        }
        if truncated { break; }
    }
    Ok((truncated || !pending.is_empty(), 0))
}

pub(super) fn desktop_entry(resource: &ResourceBroker) -> Result<crate::core::files::Entry, FsError> {
    if native_vfs::shared_provider_id(&resource.path) == Some("core:local-fs") {
        resource.scope.authorize(&resource.path)?;
        return crate::core::files::entry(std::path::Path::new(&resource.path)).map_err(FsError::from);
    }
    match resolve(&resource.path)? {
        Route::NativeProvider => Ok(native_vfs::stat_for(resource).map_err(FsError::from)?),
        Route::Plugin => plugin_stat(resource),
    }
}

pub(super) fn desktop_listing(
    resource: &ResourceBroker,
    hidden: bool,
    ignores: &[String],
) -> Result<crate::core::files::Listing, FsError> {
    if native_vfs::shared_provider_id(&resource.path) == Some("core:local-fs") {
        resource.scope.authorize(&resource.path)?;
        return crate::core::files::list(&resource.path, hidden, ignores).map_err(FsError::from);
    }
    let entries = match resolve(&resource.path)? {
        Route::NativeProvider => native_vfs::list_for(resource).map_err(FsError::from)?,
        Route::Plugin => plugin_list(resource)?,
    };
    let filter = crate::core::ignores::IgnoreSet::new(ignores);
    let mut entries = entries.into_iter().filter(|entry| {
        (hidden || !entry.name.starts_with('.'))
            && !filter.is_ignored(std::path::Path::new(&entry.name))
    }).collect::<Vec<_>>();
    entries.sort_by_cached_key(|entry| (!entry.is_dir, entry.name.to_lowercase()));
    let mutable = [
        (crate::plugins::provider_registry::FsOperation::Write, false),
        (crate::plugins::provider_registry::FsOperation::Mkdir, true),
        (crate::plugins::provider_registry::FsOperation::Rename, false),
        (crate::plugins::provider_registry::FsOperation::Remove, false),
    ]
    .into_iter()
    .any(|(operation, create)| require(resource, operation, create).is_ok());
    let read_only = (native_vfs::provider_id(&resource.path).is_some()
        && native_vfs::readonly(&resource.path))
        || !mutable;
    Ok(crate::core::files::Listing {
        path: resource.path.clone(),
        entries,
        read_only,
        can_create: require(resource, crate::plugins::provider_registry::FsOperation::Mkdir, true).is_ok(),
        virtual_volume: true,
        warnings: Vec::new(),
        default_view: native_vfs::default_view(&resource.path).map(str::to_owned),
    })
}

pub(super) fn native_export_path(resource: &ResourceBroker) -> Option<std::path::PathBuf> {
    (native_vfs::shared_provider_id(&resource.path) == Some("core:local-fs"))
        .then(|| std::path::PathBuf::from(&resource.path))
}

pub(super) fn move_to_trash(resource: &ResourceBroker) -> Result<(), FsError> {
    if native_vfs::shared_provider_id(&resource.path) == Some("core:local-fs") {
        let mut result = crate::core::operations::recycle(std::slice::from_ref(&resource.path))?;
        if let Some(error) = result.errors.pop() { return Err(error.into()); }
        return Ok(());
    }
    remove(resource)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_virtual_directories_are_native_storage_providers() {
        for path in ["recent://", "applications://"] {
            assert!(matches!(resolve(path).unwrap(), Route::NativeProvider), "{path}");
        }
    }

    #[test]
    fn unknown_and_revoked_uri_schemes_cannot_become_local_fs() {
        for path in ["unknown://not-a-file", "unknown:opaque-resource", "fiex:unregistered", "fiex://mount/nonexistent/resource", ""] {
            assert!(matches!(resolve(path), Ok(Route::Plugin) | Err(_)), "{path}");
        }
    }

    #[test]
    fn uri_detection_preserves_windows_drive_paths() {
        assert!(!unregistered_scheme(r"C:\\Users\\Example"));
        assert!(!unregistered_scheme("D:/Documents"));
        assert!(unregistered_scheme("unknown:opaque"));
        assert!(unregistered_scheme("web+custom:resource"));
    }

    #[test]
    fn real_host_paths_use_the_registered_native_provider() {
        let path = std::env::temp_dir().join("fiex-provider-registry-local");
        assert!(matches!(resolve(&path.to_string_lossy()), Ok(Route::NativeProvider)));
    }
}
