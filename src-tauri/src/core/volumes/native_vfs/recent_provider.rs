//! Core adapters for the OS, scoped-resource, and cache operations used by the
//! Recent module. Recent's VFS, hook, and menu policies live in `fiex-recent`.
use crate::{
    core::{files::Entry, fs::mapped_access::OperationScope},
    plugins::{
        native_invocation::NativeProviderInvocation, resource::resource_context::ResourceSnapshot,
    },
};
use fiex_provider_contracts::{FsRead, ProviderError, ProviderInvocation};
use fiex_recent::{Existence, RecentModuleServices, RecentResource, RecentStore};
use std::collections::HashMap;
use std::sync::Arc;

struct CoreRecentServices;

pub(super) fn services() -> Arc<dyn RecentModuleServices> {
    Arc::new(CoreRecentServices)
}

fn candidate(snapshot: &ResourceSnapshot) -> RecentResource {
    let metadata = snapshot.metadata().cloned().unwrap_or_default();
    RecentResource {
        locator: snapshot.display.clone(),
        name: metadata.name,
        extension: metadata.extension.unwrap_or_default(),
        mime: metadata.mime.unwrap_or_default(),
        size: metadata.size,
        modified: metadata.modified,
        is_directory: snapshot.is_dir,
    }
}

pub(super) fn source_for(alias: &str) -> Result<String, String> {
    let storage = super::management_storage_for_provider(fiex_recent::PROVIDER_ID, "history")?;
    RecentStore::new(storage).source_for(alias)
}

pub(super) fn filter_cached_entries(
    entries: &mut Vec<Entry>,
    storage: &fiex_storage::native::NativeScopedStorage,
) -> Result<(), String> {
    let by_alias = RecentStore::new(storage.clone())
        .records()?
        .into_iter()
        .map(|record| (format!("recent://{}", record.id), record))
        .collect::<HashMap<_, _>>();
    entries.retain(|entry| {
        by_alias
            .get(&entry.path)
            .is_some_and(|record| probe_existence(&record.locator) != Existence::Missing)
    });
    Ok(())
}

fn probe_existence(locator: &str) -> Existence {
    use crate::core::fs::provider_registry::ResourceOwner;
    match crate::core::fs::provider_registry::owner(locator) {
        Ok(ResourceOwner::LocalFs) => match std::fs::symlink_metadata(locator) {
            Ok(_) => Existence::Present,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Existence::Missing,
            Err(_) => Existence::Unknown,
        },
        Ok(ResourceOwner::NativeVfs)
            if crate::core::volumes::native_vfs::is_filesystem_path(locator) =>
        {
            match crate::core::fs::provider_registry::resource_entry(locator) {
                Ok(_) => Existence::Present,
                Err(error) if error.contains("NotFound") => Existence::Missing,
                Err(_) => Existence::Unknown,
            }
        }
        Ok(ResourceOwner::PluginMount { .. }) | Ok(ResourceOwner::NativeVfs) | Err(_) => {
            Existence::Unknown
        }
    }
}

fn normalize_locator(locator: &str, store: &RecentStore) -> Result<String, String> {
    if locator.starts_with("recent://") {
        return store.source_for(locator);
    }
    use crate::core::fs::provider_registry::ResourceOwner;
    match crate::core::fs::provider_registry::owner(locator).map_err(String::from)? {
        ResourceOwner::LocalFs => std::fs::canonicalize(locator)
            .map(|path| path.to_string_lossy().into_owned())
            .or_else(|_| Ok(locator.to_owned())),
        ResourceOwner::PluginMount { .. } => Ok(locator.to_owned()),
        ResourceOwner::NativeVfs
            if crate::core::volumes::native_vfs::is_filesystem_path(locator) =>
        {
            Ok(locator.to_owned())
        }
        ResourceOwner::NativeVfs => Err("resource cannot be saved in Recent".into()),
    }
}

struct BrokerReader {
    handle: Box<dyn crate::core::fs::ReadHandle>,
    offset: u64,
}

impl std::io::Read for BrokerReader {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let (bytes, eof) = self
            .handle
            .read_at(self.offset, output.len().min(crate::core::fs::CHUNK))
            .map_err(std::io::Error::other)?;
        if bytes.is_empty() && !eof {
            return Err(std::io::Error::other("source stream stalled"));
        }
        output[..bytes.len()].copy_from_slice(&bytes);
        self.offset = self
            .offset
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| std::io::Error::other("read offset overflow"))?;
        Ok(bytes.len())
    }
}

impl RecentModuleServices for CoreRecentServices {
    fn storage(
        &self,
        ctx: &dyn ProviderInvocation,
        scope: &str,
    ) -> Result<fiex_storage::native::NativeScopedStorage, ProviderError> {
        NativeProviderInvocation::storage(ctx, scope)
    }

    fn path(&self, ctx: &dyn ProviderInvocation) -> Result<String, ProviderError> {
        super::shared::path(ctx).map(str::to_owned)
    }

    fn resource_for_open(
        &self,
        ctx: &dyn ProviderInvocation,
    ) -> Result<RecentResource, ProviderError> {
        NativeProviderInvocation::resource(ctx, 0).map(candidate)
    }

    fn normalize_locator(&self, locator: &str, store: &RecentStore) -> Result<String, String> {
        normalize_locator(locator, store)
    }

    fn existence(&self, locator: &str) -> Existence {
        probe_existence(locator)
    }

    fn open_source(
        &self,
        ctx: &dyn ProviderInvocation,
        locator: &str,
    ) -> Result<FsRead, ProviderError> {
        let operation_scope = OperationScope::new(super::shared::broker(ctx)?);
        let source = crate::core::fs::mapped_access::authorize_source(
            locator,
            &operation_scope,
            fiex_provider_contracts::FsOperation::Read,
        )
        .map_err(provider_error)?;
        let size = source
            .stat()
            .map_err(provider_error)?
            .get("size")
            .and_then(serde_json::Value::as_u64);
        let handle = source.open_read_handle().map_err(provider_error)?;
        Ok(FsRead::new(BrokerReader { handle, offset: 0 }, size))
    }

    fn selection(
        &self,
        ctx: &dyn ProviderInvocation,
    ) -> Result<(String, Vec<String>), ProviderError> {
        let directory = NativeProviderInvocation::directory(ctx)?.display.clone();
        let paths = (0..ctx.targets().len())
            .map(|index| {
                NativeProviderInvocation::resource(ctx, index)
                    .map(|resource| resource.display.clone())
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok((directory, paths))
    }

    fn invalidate_removed_aliases(&self, aliases: &[String]) {
        for alias in aliases {
            crate::core::fs::changes::removed_from(alias, "recent://");
        }
    }
}

fn provider_error(error: impl ToString) -> ProviderError {
    let message = error.to_string();
    if message.starts_with("Unsupported:") {
        ProviderError::method_not_found("vfs.operation")
    } else {
        ProviderError::invalid_params(message)
    }
}
