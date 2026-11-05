//! Trash view implementation of the shared VFS provider contract.
use super::{recycle, shared, shared::search_listing, trash_vfs};
use crate::plugins::native_invocation::NativeProviderInvocation;
use fiex_provider_contracts::{FsListing, FsRequest, ProviderError, ProviderInvocation};
use fiex_trash::TrashModuleServices;
use std::sync::Arc;

fn trash_list(path: &str) -> Result<Vec<crate::core::files::Entry>, String> {
    if trash_vfs::handles(path) {
        trash_vfs::list(path)
    } else if path == recycle::URI {
        recycle::list()
    } else {
        Err("Unsupported: builtin node has no children".into())
    }
}

pub(super) fn services() -> Arc<dyn TrashModuleServices> {
    Arc::new(NativeTrashServices)
}

struct NativeTrashServices;

impl TrashModuleServices for NativeTrashServices {
    fn path(&self, ctx: &dyn ProviderInvocation) -> Result<String, ProviderError> {
        shared::path(ctx).map(str::to_owned)
    }

    fn search(&self, root: &str, request: &FsRequest<'_>) -> Result<FsListing, ProviderError> {
        search_listing(root, request, trash_list)
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

    fn map_error(&self, error: String) -> String {
        trash_vfs::map_error(error)
    }
}
