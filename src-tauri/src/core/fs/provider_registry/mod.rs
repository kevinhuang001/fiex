//! Core-owned dispatch into the registered storage providers.
//!
//! Plugin mount adaptation is isolated in `plugin`; owner and identity
//! classification is isolated in `routing`.
use super::{
    handles::{ProviderReadHandle, ProviderWriteHandle},
    FsError, FsErrorKind, ReadHandle, ResourceBroker, WriteHandle,
};
use crate::{
    core::volumes::native_vfs,
    plugins::provider_registry::FsOperation,
};
use fiex_provider_contracts::{FsWrite, TransferPreflight};
use serde_json::{to_value, Value};
mod plugin;
mod routing;
use routing::{resolve, Route};
pub(crate) use routing::{
    child_path, identity, is_plugin_mount, owner, parent_path, reference_mount_id,
    resource_access, resource_entry, validate_child, ResourceAccess, ResourceOwner,
};

pub(super) fn require(
    resource: &ResourceBroker,
    operation: FsOperation,
    create: bool,
) -> Result<(), FsError> {
    resource.ensure_current_binding()?;
    match resolve(&resource.path)? {
        Route::NativeProvider => {
            native_vfs::require_for(resource, operation, create).map_err(FsError::from)
        }
        Route::Plugin => {
            let (mount, node) = if create {
                let (mount, node, _) = plugin::parent(resource)?;
                (mount, node)
            } else {
                plugin::plugin_node(resource)?
            };
            let required = if create
                && matches!(operation, FsOperation::Write | FsOperation::Mkdir)
            {
                FsOperation::Mkdir
            } else if operation == FsOperation::Search
                && !mount.supports(FsOperation::Search)
            {
                FsOperation::List
            } else {
                operation
            };
            if !mount.supports(required) {
                return Err(FsError::new(
                    FsErrorKind::Unsupported,
                    format!("Unsupported: {}", required.as_str()),
                ));
            }
            let entry = mount.stat(&node).map_err(FsError::from)?;
            if !entry.allows(required) {
                return Err(FsError::new(
                    FsErrorKind::Unsupported,
                    format!("Unsupported: node does not permit {}", required.as_str()),
                ));
            }
            Ok(())
        }
    }
}

pub(super) fn stat(resource: &ResourceBroker) -> Result<Value, FsError> {
    let entry = match resolve(&resource.path)? {
        Route::NativeProvider => native_vfs::stat_for(resource).map_err(FsError::from)?,
        Route::Plugin => plugin::stat(resource)?,
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
        Route::Plugin => plugin::list(resource)?,
    };
    to_value(entries).map_err(|error| FsError::internal(error.to_string()))
}

pub(super) fn open_read(resource: &ResourceBroker) -> Result<Box<dyn ReadHandle>, FsError> {
    require(resource, FsOperation::Read, false)?;
    let read = match resolve(&resource.path)? {
        Route::NativeProvider => native_vfs::read_for(resource).map_err(FsError::from)?,
        Route::Plugin => plugin::stream_read(resource)?,
    };
    Ok(Box::new(ProviderReadHandle::new(read)?))
}

pub(super) fn write_stream(
    resource: &ResourceBroker,
    create: bool,
    size: Option<u64>,
) -> Result<FsWrite, FsError> {
    require(resource, FsOperation::Write, create)?;
    match resolve(&resource.path)? {
        Route::NativeProvider => {
            native_vfs::write_sized_for(resource, create, size).map_err(FsError::from)
        }
        Route::Plugin => plugin::stream_write(resource, create, size),
    }
}

pub(super) fn begin_write(
    resource: &ResourceBroker,
    create: bool,
) -> Result<Box<dyn WriteHandle>, FsError> {
    require(resource, FsOperation::Write, create)?;
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
    require(resource, FsOperation::Mkdir, true)?;
    let entry = match resolve(&resource.path)? {
        Route::NativeProvider => native_vfs::mkdir_for(resource)
            .map(native_vfs::core_entry)
            .map_err(FsError::from)?,
        Route::Plugin => {
            let (mount, parent, name) = plugin::parent(resource)?;
            let entry = mount.mkdir(&parent, &name).map_err(FsError::from)?;
            plugin::remember_entry(&resource.path, &entry)?;
            plugin::project_entry(&mount, &resource.path, entry)
        }
    };
    to_value(entry).map_err(|error| FsError::internal(error.to_string()))
}

pub(super) fn remove(resource: &ResourceBroker) -> Result<(), FsError> {
    require(resource, FsOperation::Remove, false)?;
    match resolve(&resource.path)? {
        Route::NativeProvider => native_vfs::remove_for(resource).map_err(FsError::from),
        Route::Plugin => {
            let (mount, node) = plugin::plugin_node(resource)?;
            mount.remove(&node).map_err(FsError::from)
        }
    }
}

pub(super) fn rename(source: &ResourceBroker, destination: &ResourceBroker) -> Result<(), FsError> {
    require(source, FsOperation::Rename, false)?;
    require(destination, FsOperation::Write, true)?;
    if source.reference.mount_id != destination.reference.mount_id {
        return Err("Unsupported: cross-mount rename requires TransferEngine".into());
    }
    match resolve(&source.path)? {
        Route::NativeProvider => native_vfs::rename_for(source, destination)
            .map(|_| ())
            .map_err(FsError::from),
        Route::Plugin => {
            let (mount, node) = plugin::plugin_node(source)?;
            let (destination_mount, parent, name) = plugin::parent(destination)?;
            if mount.mount_id != destination_mount.mount_id {
                return Err("Unsupported: cross-mount rename requires TransferEngine".into());
            }
            let entry = mount.rename(&node, &name, Some(&parent)).map_err(FsError::from)?;
            plugin::remember_entry(&destination.path, &entry)
        }
    }
}

pub(super) fn preflight(
    resource: &ResourceBroker,
    request: &TransferPreflight,
) -> Result<(), FsError> {
    match resolve(&resource.path)? {
        Route::NativeProvider => {
            native_vfs::preflight_for(resource, request).map_err(FsError::from)
        }
        Route::Plugin => {
            let (mount, node) = plugin::plugin_node(resource)?;
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
    let (truncated, skipped) = search_stream(
        resource,
        query,
        hidden,
        ignores,
        limit,
        cancelled,
        &mut |entry| entries.push(entry),
    )?;
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
    require(resource, FsOperation::Search, false)?;
    match resolve(&resource.path)? {
        Route::NativeProvider => {
            let (entries, truncated) = native_vfs::search_for(
                resource, query, hidden, ignores, limit, cancelled,
            )
            .map_err(FsError::from)?;
            for entry in entries {
                if cancelled() {
                    break;
                }
                emit(entry);
            }
            Ok((truncated, 0))
        }
        Route::Plugin if !crate::remote::is_remote(&resource.path) => {
            let (mount, _) = plugin::plugin_node(resource)?;
            if mount.supports(FsOperation::Search) {
                let truncated = crate::plugins::vfs::search_stream(
                    &resource.path, query, hidden, ignores, limit, cancelled, emit,
                ).map_err(FsError::from)?;
                return Ok((truncated, 0));
            }
            plugin::walk_search(resource, query, hidden, ignores, limit, cancelled, emit)
        }
        Route::Plugin => {
            plugin::walk_search(resource, query, hidden, ignores, limit, cancelled, emit)
        }
    }
}

pub(super) fn desktop_entry(
    resource: &ResourceBroker,
) -> Result<crate::core::files::Entry, FsError> {
    match resolve(&resource.path)? {
        Route::NativeProvider => Ok(native_vfs::stat_for(resource).map_err(FsError::from)?),
        Route::Plugin => plugin::stat(resource),
    }
}

pub(super) fn desktop_listing(
    resource: &ResourceBroker,
    hidden: bool,
    ignores: &[String],
) -> Result<crate::core::files::Listing, FsError> {
    let entries = match resolve(&resource.path)? {
        Route::NativeProvider => native_vfs::list_for(resource).map_err(FsError::from)?,
        Route::Plugin => plugin::list(resource)?,
    };
    let filter = crate::core::ignores::IgnoreSet::new(ignores);
    let mut entries = entries.into_iter().filter(|entry| {
        (hidden || !entry.name.starts_with('.'))
            && !filter.is_ignored(std::path::Path::new(&entry.name))
    }).collect::<Vec<_>>();
    entries.sort_by_cached_key(|entry| (!entry.is_dir, entry.name.to_lowercase()));
    let mutable = [
        (FsOperation::Write, false),
        (FsOperation::Mkdir, true),
        (FsOperation::Rename, false),
        (FsOperation::Remove, false),
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
        can_create: require(resource, FsOperation::Mkdir, true).is_ok(),
        virtual_volume: native_vfs::shared_provider_id(&resource.path) != Some("core:local-fs"),
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
        if let Some(error) = result.errors.pop() {
            return Err(error.into());
        }
        return Ok(());
    }
    remove(resource)
}
