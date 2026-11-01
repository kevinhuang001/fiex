//! Trash view implementation of the shared VFS provider contract.
use super::shared::*;
use super::*;

fn trash_operations(path: &str) -> Vec<FsOperation> {
    match storage::trash_stat(
        path,
        &[FsOperation::List, FsOperation::Stat, FsOperation::Search],
    ) {
        Ok(entry) if entry.is_dir && !entry.is_symlink => {
            vec![FsOperation::List, FsOperation::Stat, FsOperation::Search]
        }
        Ok(_) => vec![FsOperation::Stat],
        Err(_) => Vec::new(),
    }
}

pub(super) struct TrashView;
impl VfsProvider for TrashView {
    fn supports_preflight(&self) -> bool {
        true
    }

    fn operations(&self) -> &'static [FsOperation] {
        &[FsOperation::List, FsOperation::Stat, FsOperation::Search]
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
            FsOperation::Write | FsOperation::Mkdir | FsOperation::Rename | FsOperation::Remove
        ) {
            return Err(ProviderError::invalid_params(
                "PermissionDenied: Trash requires dedicated restore/delete actions",
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

    fn search_capabilities(&self, ctx: &dyn ProviderInvocation) -> SearchCapabilities {
        SearchCapabilities {
            name: path(ctx).is_ok_and(|path| trash_operations(path).contains(&FsOperation::Search)),
            content: false,
        }
    }

    fn search(&self, request: &FsRequest<'_>) -> Result<FsListing, ProviderError> {
        search_listing(path(request.ctx())?, request, storage::trash_list)
    }

    fn preflight(
        &self,
        _ctx: &dyn ProviderInvocation,
        _request: &TransferPreflight,
    ) -> Result<fiex_provider_contracts::PreflightDecision, ProviderError> {
        Err(ProviderError::invalid_params(
            "PermissionDenied: Trash requires dedicated restore/delete actions",
        ))
    }
}
