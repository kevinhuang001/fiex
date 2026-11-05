//! Trash view implementation of the shared VFS provider contract.
use super::shared::*;
use super::*;
use crate::plugins::native_invocation::NativeProviderInvocation;
use fiex_trash::TrashModuleServices;
use std::sync::Arc;

pub(super) fn services() -> Arc<dyn TrashModuleServices> {
    Arc::new(NativeTrashServices)
}

struct NativeTrashServices;

impl TrashModuleServices for NativeTrashServices {
    fn path(&self, ctx: &dyn ProviderInvocation) -> Result<String, ProviderError> {
        super::shared::path(ctx).map(str::to_owned)
    }

    fn capabilities(&self, path: &str) -> Vec<FsOperation> {
        if trash_vfs::handles(path) {
            trash_vfs::entry(path)
        } else if recycle::is_virtual(path) {
            recycle::stat(path)
        } else {
            return Vec::new();
        }
        .map(|entry| {
            entry
                .allowed_ops
                .iter()
                .filter_map(|operation| FsOperation::parse(operation))
                .collect()
        })
        .unwrap_or_default()
    }

    fn stat(&self, path: &str, operations: &[FsOperation]) -> Result<FsEntry, ProviderError> {
        storage::trash_stat(path, operations)
            .map(shared_entry)
            .map_err(provider_error)
    }

    fn list(&self, path: &str) -> Result<FsListing, ProviderError> {
        storage::trash_list(path)
            .map(list_result)
            .map_err(provider_error)
    }

    fn purge(&self, path: &str) -> Result<(), ProviderError> {
        if trash_vfs::handles(path) {
            trash_vfs::purge(path).map_err(provider_error)
        } else if let Some(identity) = path.strip_prefix(recycle::URI) {
            recycle::purge(identity).map_err(provider_error)
        } else {
            Err(ProviderError::invalid_params(
                "ScopeDenied: Trash root cannot be permanently removed",
            ))
        }
    }

    fn search(&self, root: &str, request: &FsRequest<'_>) -> Result<FsListing, ProviderError> {
        search_listing(root, request, storage::trash_list)
    }

    fn selection(
        &self,
        ctx: &dyn ProviderInvocation,
    ) -> Result<(String, Vec<String>), ProviderError> {
        let directory = NativeProviderInvocation::directory(ctx)?.display.clone();
        let paths = NativeProviderInvocation::resources(ctx)?
            .iter()
            .map(|resource| resource.display.clone())
            .collect();
        Ok((directory, paths))
    }

    fn translate(&self, text: &'static str) -> String {
        crate::i18n::t(text).into()
    }
}
