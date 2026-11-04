//! Trash view implementation of the shared VFS provider contract.
use super::shared::*;
use super::*;

fn trash_operations(path: &str) -> Vec<FsOperation> {
    if path == trash_vfs::URI || path == recycle::URI {
        return vec![FsOperation::List, FsOperation::Stat, FsOperation::Search];
    }
    match storage::trash_stat(
        path,
        &[FsOperation::List, FsOperation::Stat, FsOperation::Search],
    ) {
        Ok(entry) if entry.is_dir && !entry.is_symlink => {
            vec![
                FsOperation::List,
                FsOperation::Stat,
                FsOperation::Search,
                FsOperation::Remove,
            ]
        }
        Ok(_) => vec![FsOperation::Stat, FsOperation::Remove],
        Err(_) => Vec::new(),
    }
}

pub(super) struct TrashView;
impl VfsProvider for TrashView {
    fn operations(&self) -> &'static [FsOperation] {
        &[
            FsOperation::List,
            FsOperation::Stat,
            FsOperation::Search,
            FsOperation::Remove,
        ]
    }

    fn discover(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<FsOperation>, ProviderError> {
        Ok(trash_operations(path(ctx)?))
    }

    fn authorize(
        &self,
        ctx: &dyn ProviderInvocation,
        operation: FsOperation,
        _create: bool,
    ) -> Result<(), ProviderError> {
        if matches!(
            operation,
            FsOperation::Write | FsOperation::Mkdir | FsOperation::Rename | FsOperation::Trash
        ) {
            return Err(ProviderError::invalid_params(
                "PermissionDenied: Trash only supports controlled permanent removal",
            ));
        }
        if trash_operations(path(ctx)?).contains(&operation) {
            Ok(())
        } else {
            Err(ProviderError::method_not_found("vfs.operation"))
        }
    }

    fn stat(&self, request: &FsRequest<'_>) -> Result<FsEntry, ProviderError> {
        let path = path(request.ctx())?;
        storage::trash_stat(path, &trash_operations(path))
            .map(shared_entry)
            .map_err(provider_error)
    }

    fn list(&self, request: &FsRequest<'_>) -> Result<FsListing, ProviderError> {
        storage::trash_list(path(request.ctx())?)
            .map(list_result)
            .map_err(provider_error)
    }

    fn remove(&self, request: &FsRequest<'_>) -> Result<(), ProviderError> {
        let uri = path(request.ctx())?;
        if request.node_id() != Some(uri) {
            return Err(ProviderError::invalid_params(
                "nodeId does not match scoped Trash resource",
            ));
        }
        self.authorize(request.ctx(), FsOperation::Remove, false)?;
        if trash_vfs::handles(uri) {
            trash_vfs::purge(uri).map_err(provider_error)
        } else if let Some(identity) = uri.strip_prefix(recycle::URI) {
            recycle::purge(identity).map_err(provider_error)
        } else {
            Err(ProviderError::invalid_params(
                "ScopeDenied: Trash root cannot be permanently removed",
            ))
        }
    }

    fn search_capabilities(&self, ctx: &dyn ProviderInvocation) -> SearchCapabilities {
        SearchCapabilities {
            name: path(ctx).is_ok_and(|path| trash_operations(path).contains(&FsOperation::Search)),
            content: false,
        }
    }

    fn search(&self, request: &FsRequest<'_>) -> Result<FsListing, ProviderError> {
        search_listing(path(request.ctx())?, request, storage::trash_list)
    }
}
