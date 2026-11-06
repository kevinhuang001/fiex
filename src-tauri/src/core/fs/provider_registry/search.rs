use super::{
    plugin, require,
    routing::{resolve, Route},
    FsError, ResourceBroker,
};
use crate::core::volumes::native_vfs;
use fiex_provider_contracts::FsOperation;

pub(in crate::core::fs) fn search(
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

pub(in crate::core::fs) fn search_stream(
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
            let (entries, truncated) =
                native_vfs::search_for(resource, query, hidden, ignores, limit, cancelled)
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
                    &resource.path,
                    query,
                    hidden,
                    ignores,
                    limit,
                    cancelled,
                    emit,
                )
                .map_err(FsError::from)?;
                return Ok((truncated, 0));
            }
            plugin::walk_search(resource, query, hidden, ignores, limit, cancelled, emit)
        }
        Route::Plugin => {
            plugin::walk_search(resource, query, hidden, ignores, limit, cancelled, emit)
        }
    }
}
