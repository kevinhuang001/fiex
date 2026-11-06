use super::{
    plugin, require,
    routing::{resolve, Route},
    FsError, ProviderReadHandle, ReadHandle, ResourceBroker,
};
use crate::core::volumes::native_vfs;
use fiex_provider_contracts::FsOperation;
use serde_json::{to_value, Value};

pub(in crate::core::fs) fn stat(resource: &ResourceBroker) -> Result<Value, FsError> {
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

pub(in crate::core::fs) fn list(resource: &ResourceBroker) -> Result<Value, FsError> {
    let entries = match resolve(&resource.path)? {
        Route::NativeProvider => native_vfs::list_for(resource).map_err(FsError::from)?,
        Route::Plugin => plugin::list(resource)?.0,
    };
    to_value(entries).map_err(|error| FsError::internal(error.to_string()))
}

pub(in crate::core::fs) fn open_read(
    resource: &ResourceBroker,
) -> Result<Box<dyn ReadHandle>, FsError> {
    require(resource, FsOperation::Read, false)?;
    let read = match resolve(&resource.path)? {
        Route::NativeProvider => native_vfs::read_for(resource).map_err(FsError::from)?,
        Route::Plugin => plugin::stream_read(resource)?,
    };
    Ok(Box::new(ProviderReadHandle::new(read)?))
}

pub(in crate::core::fs) fn desktop_entry(
    resource: &ResourceBroker,
) -> Result<crate::core::files::Entry, FsError> {
    match resolve(&resource.path)? {
        Route::NativeProvider => Ok(native_vfs::stat_for(resource).map_err(FsError::from)?),
        Route::Plugin => plugin::stat(resource),
    }
}

pub(in crate::core::fs) fn desktop_listing(
    resource: &ResourceBroker,
    hidden: bool,
    ignores: &[String],
) -> Result<crate::core::files::Listing, FsError> {
    let (entries, warnings) = match resolve(&resource.path)? {
        Route::NativeProvider => (
            native_vfs::list_for(resource).map_err(FsError::from)?,
            Vec::new(),
        ),
        Route::Plugin => plugin::list(resource)?,
    };
    let filter = crate::core::ignores::IgnoreSet::new(ignores);
    let mut entries = entries
        .into_iter()
        .filter(|entry| {
            (hidden || !entry.name.starts_with('.'))
                && !filter.is_ignored(std::path::Path::new(&entry.name))
        })
        .collect::<Vec<_>>();
    entries.sort_by_cached_key(|entry| (!entry.is_dir, entry.name.to_lowercase()));
    // The directory's declared capabilities drive affordances. Do not probe
    // host ACLs while listing; the OS reports access failures on mutation.
    let allowed_ops = match resolve(&resource.path)? {
        Route::NativeProvider => {
            native_vfs::stat_for(resource)
                .map_err(FsError::from)?
                .allowed_ops
        }
        Route::Plugin => plugin::stat(resource)?.allowed_ops,
    };
    Ok(crate::core::files::Listing {
        path: resource.path.clone(),
        entries,
        allowed_ops,
        virtual_volume: native_vfs::shared_provider_id(&resource.path) != Some("core:local-fs"),
        warnings,
        default_view: native_vfs::default_view(&resource.path).map(str::to_owned),
    })
}

pub(in crate::core::fs) fn native_export_path(
    resource: &ResourceBroker,
) -> Option<std::path::PathBuf> {
    (native_vfs::shared_provider_id(&resource.path) == Some("core:local-fs"))
        .then(|| std::path::PathBuf::from(&resource.path))
}
