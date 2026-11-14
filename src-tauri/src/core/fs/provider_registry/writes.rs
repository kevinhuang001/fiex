use super::{
    FsError, ProviderWriteHandle, ResourceBroker, WriteHandle, plugin, require,
    routing::{Route, resolve},
};
use crate::core::fs::handles::StagedWriteHandle;
use crate::core::volumes::native_vfs;
use fiex_provider_contracts::{
    FsOperation, FsWrite, PreflightOperation, PreflightPhase, PreflightRequest,
};
use serde_json::{Value, to_value};

#[cfg(test)]
thread_local! {
    static NEXT_PROVIDER_WRITE_FAILURES: std::cell::RefCell<std::collections::VecDeque<String>> =
        const { std::cell::RefCell::new(std::collections::VecDeque::new()) };
}

#[cfg(test)]
pub(crate) fn fail_next_provider_write_for_test(path: &str) {
    fail_provider_writes_for_test(&[path.to_owned()]);
}

#[cfg(test)]
pub(crate) fn fail_provider_writes_for_test(paths: &[String]) {
    NEXT_PROVIDER_WRITE_FAILURES.with(|failures| {
        *failures.borrow_mut() = paths.iter().cloned().collect();
    });
}

#[cfg(test)]
pub(crate) fn clear_provider_write_failures_for_test() {
    NEXT_PROVIDER_WRITE_FAILURES.with(|failures| failures.borrow_mut().clear());
}

#[cfg(test)]
fn injected_write_failure(resource: &ResourceBroker) -> Option<FsWrite> {
    let should_fail = NEXT_PROVIDER_WRITE_FAILURES.with(|failures| {
        let mut failures = failures.borrow_mut();
        if failures.front().is_some_and(|path| path == &resource.path) {
            failures.pop_front();
            true
        } else {
            false
        }
    });
    should_fail.then(|| {
        struct FailingProviderWriter;
        impl std::io::Write for FailingProviderWriter {
            fn write(&mut self, _buffer: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("injected provider write failure"))
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        FsWrite::new(FailingProviderWriter)
    })
}

pub(in crate::core::fs) fn write_stream(
    resource: &ResourceBroker,
    create: bool,
    size: Option<u64>,
) -> Result<FsWrite, FsError> {
    require(resource, FsOperation::Write, create)?;
    if create {
        resource.review_create_child(false, PreflightPhase::Execute)?;
    }
    #[cfg(test)]
    if let Some(failure) = injected_write_failure(resource) {
        return Ok(failure);
    }
    match resolve(&resource.path)? {
        Route::NativeProvider => {
            native_vfs::write_sized_for(resource, create, size).map_err(FsError::from)
        }
        Route::Plugin => plugin::stream_write(resource, create, size),
    }
}

pub(in crate::core::fs) fn begin_write(
    resource: &ResourceBroker,
    create: bool,
) -> Result<Box<dyn WriteHandle>, FsError> {
    require(resource, FsOperation::Write, create)?;
    if create {
        resource.review_create_child(false, PreflightPhase::Execute)?;
    }
    Ok(Box::new(StagedWriteHandle::new(resource, create)?))
}

pub(in crate::core::fs) fn begin_write_sized(
    resource: &ResourceBroker,
    create: bool,
    size: Option<u64>,
) -> Result<Box<dyn WriteHandle>, FsError> {
    let stream = write_stream(resource, create, size)?;
    Ok(Box::new(ProviderWriteHandle::new(stream)))
}

pub(in crate::core::fs) fn mkdir(resource: &ResourceBroker) -> Result<Value, FsError> {
    require(resource, FsOperation::Mkdir, true)?;
    resource.review_create_child(true, PreflightPhase::Execute)?;
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

pub(in crate::core::fs) fn remove(resource: &ResourceBroker) -> Result<(), FsError> {
    require(resource, FsOperation::Remove, false)?;
    resource.review_remove(PreflightPhase::Execute)?;
    match resolve(&resource.path)? {
        Route::NativeProvider => native_vfs::remove_for(resource).map_err(FsError::from),
        Route::Plugin => {
            let (mount, node) = plugin::plugin_node(resource)?;
            mount.remove(&node).map_err(FsError::from)
        }
    }
}

pub(in crate::core::fs) fn rename(
    source: &ResourceBroker,
    destination: &ResourceBroker,
) -> Result<String, FsError> {
    require(source, FsOperation::Rename, false)?;
    require(destination, FsOperation::Write, true)?;
    if source.reference.mount_id != destination.reference.mount_id {
        return Err("Unsupported: cross-mount rename requires TransferEngine".into());
    }
    let target_name = destination
        .path
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| {
            FsError::new(
                crate::core::fs::FsErrorKind::InvalidVfsUri,
                "missing target name",
            )
        })?;
    let source_parent = crate::core::fs::parent_path(&source.path)?;
    let destination_parent = crate::core::fs::parent_path(&destination.path)?;
    if source_parent == destination_parent {
        source.review_rename(Some(target_name), PreflightPhase::Execute)?;
    } else {
        let target_parent = destination.parent_broker()?;
        source.review_move(&target_parent, Some(target_name), PreflightPhase::Execute)?;
    }
    match resolve(&source.path)? {
        Route::NativeProvider => native_vfs::rename_for(source, destination)
            .map(|entry| entry.node_id)
            .map_err(FsError::from),
        Route::Plugin => {
            let (mount, node) = plugin::plugin_node(source)?;
            let (destination_mount, parent, name) = plugin::parent(destination)?;
            if mount.mount_id != destination_mount.mount_id {
                return Err("Unsupported: cross-mount rename requires TransferEngine".into());
            }
            let entry = mount
                .rename(&node, &name, Some(&parent))
                .map_err(FsError::from)?;
            plugin::remember_entry(&destination.path, &entry)?;
            Ok(destination.path.clone())
        }
    }
}

pub(in crate::core::fs) fn preflight(
    resource: &ResourceBroker,
    destination: Option<&ResourceBroker>,
    request: &PreflightRequest,
) -> Result<(), FsError> {
    match resolve(&resource.path)? {
        Route::NativeProvider => {
            native_vfs::preflight_for(resource, destination, request).map_err(FsError::from)
        }
        Route::Plugin => {
            let (mount, node) = plugin::plugin_node(resource)?;
            let mut request = request.clone();
            match &mut request.operation {
                PreflightOperation::CreateFile(args) => args.parent_id = node,
                PreflightOperation::CreateFolder(args) => args.parent_id = node,
                PreflightOperation::Import(args) => args.parent_id = node,
                PreflightOperation::Rename(args) => args.node_id = node,
                PreflightOperation::Trash(args) => args.node_ids = vec![node],
                PreflightOperation::Export(args) => args.node_id = node,
                PreflightOperation::Remove(args) => args.node_ids = vec![node],
                PreflightOperation::Move(args) => {
                    let destination = destination.ok_or_else(|| {
                        FsError::new(
                            crate::core::fs::FsErrorKind::InvalidVfsUri,
                            "move preflight requires a target parent",
                        )
                    })?;
                    let (target_mount, target_parent) = plugin::plugin_node(destination)?;
                    if target_mount.mount_id != mount.mount_id {
                        return Err(FsError::new(
                            crate::core::fs::FsErrorKind::InvalidVfsUri,
                            "move preflight target must belong to the same mount",
                        ));
                    }
                    args.node_id = node;
                    args.target_parent_id = target_parent;
                }
            }
            mount.preflight(&request).map_err(FsError::from)
        }
    }
}

pub(in crate::core::fs) fn move_to_trash(resource: &ResourceBroker) -> Result<(), FsError> {
    require(resource, FsOperation::Trash, false)?;
    resource.review_trash(PreflightPhase::Execute)?;
    match resolve(&resource.path)? {
        Route::NativeProvider => native_vfs::trash_for(resource).map_err(FsError::from),
        Route::Plugin => {
            let (mount, node) = plugin::plugin_node(resource)?;
            mount.trash(&node).map_err(FsError::from)
        }
    }
}
