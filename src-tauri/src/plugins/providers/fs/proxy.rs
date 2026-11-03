//! Core's persistent IPC implementation of the shared VFS provider contract.
//! Mount identity, credentials and scopes stay in the per-call invocation.
mod invocation;
mod registry;
mod stream_routes;
mod streams;

use super::Mount;
use fiex_provider_contracts::{
    FsOperation, FsRequest, ProviderError, ProviderInvocation, SearchCapabilities, VfsPage,
    VfsProvider,
};
use invocation::MountInvocation;
pub(super) use registry::forget_plugin;
use registry::provider;
use serde_json::{json, Value};
use std::sync::Arc;
pub(super) use stream_routes::{begin_write, open_read};
use streams::{RemoteReader, RemoteWriter};

struct IpcVfsProvider {
    plugin_id: String,
}

impl IpcVfsProvider {
    fn mount<'a>(&self, ctx: &'a dyn ProviderInvocation) -> Result<&'a Mount, ProviderError> {
        let mount = MountInvocation::mount(ctx)?;
        if mount.plugin_id != self.plugin_id {
            return Err(ProviderError::invalid_params("VfsProvider owner mismatch"));
        }
        Ok(mount)
    }

    fn invoke(
        &self,
        ctx: &dyn ProviderInvocation,
        method: &str,
        params: Value,
    ) -> Result<Value, ProviderError> {
        if ctx.method() != method {
            return Err(ProviderError::invalid_params("VfsProvider method mismatch"));
        }
        self.mount(ctx)?
            .invoke_remote(method, params)
            .map_err(ProviderError::internal)
    }
}

impl VfsProvider for IpcVfsProvider {
    fn operations(&self) -> &'static [FsOperation] {
        &FsOperation::ALL
    }

    fn search_capabilities(&self, ctx: &dyn ProviderInvocation) -> SearchCapabilities {
        let supports_search = self
            .mount(ctx)
            .is_ok_and(|mount| mount.supports(FsOperation::Search));
        SearchCapabilities {
            name: supports_search,
            content: false,
        }
    }

    fn discover(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<FsOperation>, ProviderError> {
        Ok(self.mount(ctx)?.operations().unwrap_or_default())
    }

    fn list(
        &self,
        request: &FsRequest<'_>,
    ) -> Result<fiex_provider_contracts::FsListing, ProviderError> {
        let result = self.invoke(request.ctx(), "vfs.list", request.to_params()?)?;
        let limit = request.limit();
        let page: fiex_provider_contracts::VfsPage = serde_json::from_value(result.clone())
            .map_err(|error| ProviderError::invalid_params(format!("invalid VFS page: {error}")))?;
        page.validate_page(limit)
            .map_err(ProviderError::invalid_params)?;
        serde_json::from_value(result)
            .map_err(|error| ProviderError::invalid_params(error.to_string()))
    }

    fn stat(
        &self,
        request: &FsRequest<'_>,
    ) -> Result<fiex_provider_contracts::FsEntry, ProviderError> {
        let result = self.invoke(request.ctx(), "vfs.stat", request.to_params()?)?;
        serde_json::from_value(result)
            .map_err(|error| ProviderError::invalid_params(error.to_string()))
    }

    fn read(
        &self,
        request: &FsRequest<'_>,
    ) -> Result<fiex_provider_contracts::FsRead, ProviderError> {
        let result = self.invoke(request.ctx(), "vfs.openRead", request.to_params()?)?;
        let stream_id = result["streamId"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| ProviderError::invalid_params("provider did not return a stream id"))?
            .to_owned();
        let mount = self.mount(request.ctx())?.clone();
        let size = result.get("size").and_then(Value::as_u64);
        Ok(fiex_provider_contracts::FsRead::new(
            RemoteReader::new(mount, stream_id),
            size,
        ))
    }

    fn write(
        &self,
        request: &FsRequest<'_>,
    ) -> Result<fiex_provider_contracts::FsWrite, ProviderError> {
        let result = self.invoke(request.ctx(), "vfs.beginWrite", request.to_params()?)?;
        let stream_id = result["streamId"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| {
                ProviderError::invalid_params("provider did not return a write stream id")
            })?
            .to_owned();
        let mount = self.mount(request.ctx())?.clone();
        let finished = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let writer = RemoteWriter::new(mount.clone(), stream_id.clone(), finished.clone());
        Ok(fiex_provider_contracts::FsWrite::with_commit(
            writer,
            move || {
                let result = mount
                    .invoke_remote("vfs.commit", json!({"streamId":stream_id}))
                    .map_err(ProviderError::internal)?;
                finished.store(true, std::sync::atomic::Ordering::Release);
                if result.is_null() || result.as_object().is_some_and(|object| object.is_empty()) {
                    return Ok(fiex_provider_contracts::FsEntry::default());
                }
                serde_json::from_value(result)
                    .map_err(|error| ProviderError::invalid_params(error.to_string()))
            },
        ))
    }

    fn mkdir(
        &self,
        request: &FsRequest<'_>,
    ) -> Result<fiex_provider_contracts::FsEntry, ProviderError> {
        let result = self.invoke(request.ctx(), "vfs.mkdir", request.to_params()?)?;
        serde_json::from_value(result)
            .map_err(|error| ProviderError::invalid_params(error.to_string()))
    }

    fn rename(
        &self,
        request: &FsRequest<'_>,
    ) -> Result<fiex_provider_contracts::FsEntry, ProviderError> {
        let result = self.invoke(request.ctx(), "vfs.rename", request.to_params()?)?;
        serde_json::from_value(result)
            .map_err(|error| ProviderError::invalid_params(error.to_string()))
    }

    fn remove(&self, request: &FsRequest<'_>) -> Result<(), ProviderError> {
        self.invoke(request.ctx(), "vfs.remove", request.to_params()?)?;
        Ok(())
    }

    fn search(
        &self,
        request: &FsRequest<'_>,
    ) -> Result<fiex_provider_contracts::FsListing, ProviderError> {
        let result = self.invoke(request.ctx(), "vfs.search", request.to_params()?)?;
        let page: VfsPage = serde_json::from_value(result)
            .map_err(|error| ProviderError::invalid_params(error.to_string()))?;
        page.validate_page(request.limit())
            .map_err(ProviderError::invalid_params)?;
        let entries = page
            .entries
            .into_iter()
            .map(serde_json::from_value)
            .collect::<Result<Vec<fiex_provider_contracts::FsEntry>, _>>()
            .map_err(|error| ProviderError::invalid_params(error.to_string()))?;
        Ok(fiex_provider_contracts::FsListing {
            entries,
            next_cursor: page.next_cursor,
            truncated: page.truncated,
            ..Default::default()
        })
    }
}

pub(super) fn invoke(mount: &Mount, method: &str, params: Value) -> Result<Value, String> {
    let provider = provider(&mount.plugin_id)?;
    let invocation = MountInvocation::new(mount.clone(), method, params.clone());
    let result = match method {
        "vfs.list" => {
            let request = FsRequest::new(&invocation, FsOperation::List)
                .map_err(|error| error.to_string())?;
            serde_json::to_value(provider.list(&request).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())
        }
        "vfs.stat" => {
            let request = FsRequest::new(&invocation, FsOperation::Stat)
                .map_err(|error| error.to_string())?;
            serde_json::to_value(provider.stat(&request).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())
        }
        "vfs.mkdir" => {
            let request = FsRequest::new(&invocation, FsOperation::Mkdir)
                .map_err(|error| error.to_string())?;
            serde_json::to_value(
                provider
                    .mkdir(&request)
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())
        }
        "vfs.rename" => {
            let request = FsRequest::new(&invocation, FsOperation::Rename)
                .map_err(|error| error.to_string())?;
            serde_json::to_value(
                provider
                    .rename(&request)
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())
        }
        "vfs.remove" => {
            let request = FsRequest::new(&invocation, FsOperation::Remove)
                .map_err(|error| error.to_string())?;
            provider
                .remove(&request)
                .map_err(|error| error.to_string())?;
            Ok(Value::Null)
        }
        "vfs.search" => {
            let request = FsRequest::new(&invocation, FsOperation::Search)
                .map_err(|error| error.to_string())?;
            serde_json::to_value(
                provider
                    .search(&request)
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())
        }
        "vfs.discover" => {
            let operations = provider
                .discover(&invocation)
                .map_err(|error| error.to_string())?;
            let search = provider.search_capabilities(&invocation);
            Ok(json!({"operations":operations,"search":search}))
        }
        // These wire methods carry transport stream IDs. The shared VFS trait
        // turns open/begin operations into Rust Read/Write streams below; the
        // chunk protocol is retained only as the proxy's IPC transport.
        "vfs.openRead" | "vfs.beginWrite" | "vfs.readStream" | "vfs.closeRead"
        | "vfs.writeStream" | "vfs.commit" | "vfs.abort" => mount.invoke_remote(method, params),
        _ => Err("unsupported VfsProvider method".into()),
    };
    result
}
