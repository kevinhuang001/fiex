use crate::{
    RECYCLE_BIN_URI, TRASH_URI, TrashModuleServices, is_recycle_bin_uri, system_recycle_bin,
    system_trash_directory,
};
use fiex_provider_contracts::{
    FsEntry, FsListing, FsOperation, FsRequest, ProviderError, ProviderInvocation,
    SearchCapabilities, VfsProvider,
};
use std::sync::Arc;

pub(super) struct TrashVfs {
    services: Arc<dyn TrashModuleServices>,
}

impl TrashVfs {
    pub(super) fn new(services: Arc<dyn TrashModuleServices>) -> Self {
        Self { services }
    }

    fn operations(&self, uri: &str) -> Vec<FsOperation> {
        if is_root(uri) {
            return vec![FsOperation::List, FsOperation::Stat, FsOperation::Search];
        }
        entry(uri, &*self.services)
            .map(|entry| entry.allowed_ops.unwrap_or_default())
            .unwrap_or_default()
    }
}

impl VfsProvider for TrashVfs {
    fn operations(&self) -> &'static [FsOperation] {
        &[
            FsOperation::List,
            FsOperation::Stat,
            FsOperation::Search,
            FsOperation::Remove,
        ]
    }

    fn discover(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<FsOperation>, ProviderError> {
        Ok(self.operations(&self.services.path(ctx)?))
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
        if self
            .operations(&self.services.path(ctx)?)
            .contains(&operation)
        {
            Ok(())
        } else {
            Err(ProviderError::method_not_found("vfs.operation"))
        }
    }

    fn stat(&self, request: &FsRequest<'_>) -> Result<FsEntry, ProviderError> {
        entry(&self.services.path(request.ctx())?, &*self.services)
    }

    fn list(&self, request: &FsRequest<'_>) -> Result<FsListing, ProviderError> {
        let uri = self.services.path(request.ctx())?;
        if uri == RECYCLE_BIN_URI {
            return system_recycle_bin()
                .list()
                .map_err(|error| provider_error(error, &*self.services));
        }
        if system_trash_directory().handles(&uri) {
            return system_trash_directory()
                .list(&uri)
                .map_err(|error| provider_error(error, &*self.services));
        }
        Err(ProviderError::invalid_params(
            "Unsupported: Trash item has no children",
        ))
    }

    fn remove(&self, request: &FsRequest<'_>) -> Result<(), ProviderError> {
        let uri = self.services.path(request.ctx())?;
        if request.node_id() != Some(uri.as_str()) {
            return Err(ProviderError::invalid_params(
                "nodeId does not match scoped Trash resource",
            ));
        }
        self.authorize(request.ctx(), FsOperation::Remove, false)?;
        if is_root(&uri) {
            return Err(ProviderError::invalid_params(
                "ScopeDenied: Trash root cannot be permanently removed",
            ));
        }
        purge(&uri, &*self.services)
    }

    fn search_capabilities(&self, ctx: &dyn ProviderInvocation) -> SearchCapabilities {
        SearchCapabilities {
            name: self
                .services
                .path(ctx)
                .is_ok_and(|uri| self.operations(&uri).contains(&FsOperation::Search)),
            content: false,
        }
    }

    fn search(&self, request: &FsRequest<'_>) -> Result<FsListing, ProviderError> {
        let root = self.services.path(request.ctx())?;
        self.services.search(&root, request)
    }
}

fn entry(uri: &str, services: &dyn TrashModuleServices) -> Result<FsEntry, ProviderError> {
    if uri == RECYCLE_BIN_URI {
        let name = if cfg!(windows) {
            "回收站"
        } else {
            "废纸篓"
        };
        return Ok(FsEntry::directory(uri, services.translate(name)).allowing([
            FsOperation::List,
            FsOperation::Stat,
            FsOperation::Search,
        ]));
    }
    if system_trash_directory().handles(uri) {
        let mut entry = system_trash_directory()
            .entry(uri)
            .map_err(|error| provider_error(error, services))?;
        if uri == TRASH_URI {
            entry.name = services.translate("废纸篓");
        }
        return Ok(entry);
    }
    if is_recycle_bin_uri(uri) {
        return system_recycle_bin()
            .stat(uri)
            .map_err(|error| provider_error(error, services));
    }
    Err(ProviderError::invalid_params(
        "Unsupported: unknown Trash resource",
    ))
}

fn purge(uri: &str, services: &dyn TrashModuleServices) -> Result<(), ProviderError> {
    if system_trash_directory().handles(uri) {
        return system_trash_directory()
            .purge(uri)
            .map_err(|error| provider_error(error, services));
    }
    if uri.starts_with(RECYCLE_BIN_URI) && uri != RECYCLE_BIN_URI {
        return system_recycle_bin()
            .purge(uri)
            .map_err(|error| provider_error(error, services));
    }
    Err(ProviderError::invalid_params(
        "ScopeDenied: Trash root cannot be permanently removed",
    ))
}

fn is_root(uri: &str) -> bool {
    uri == TRASH_URI || uri == RECYCLE_BIN_URI
}

fn provider_error(error: impl ToString, services: &dyn TrashModuleServices) -> ProviderError {
    let message = services.map_error(error.to_string());
    if message.starts_with("Unsupported:") {
        ProviderError::method_not_found("vfs.operation")
    } else {
        ProviderError::invalid_params(message)
    }
}
