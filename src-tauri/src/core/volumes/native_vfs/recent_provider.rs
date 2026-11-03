//! Recent view implementation of the shared VFS provider contract.
use super::shared::*;
use super::*;
use std::io::{self, Read};

struct BrokerReader {
    handle: Box<dyn crate::core::fs::ReadHandle>,
    offset: u64,
}

impl Read for BrokerReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let (bytes, eof) = self
            .handle
            .read_at(self.offset, output.len().min(crate::core::fs::CHUNK))
            .map_err(io::Error::other)?;
        if bytes.is_empty() && !eof {
            return Err(io::Error::other("source stream stalled"));
        }
        output[..bytes.len()].copy_from_slice(&bytes);
        self.offset = self
            .offset
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| io::Error::other("read offset overflow"))?;
        Ok(bytes.len())
    }
}

fn recent_operations(path: &str) -> Vec<FsOperation> {
    if path == RECENT_URI {
        vec![FsOperation::List, FsOperation::Stat, FsOperation::Search]
    } else {
        storage::recent_stat(path, &[FsOperation::Stat, FsOperation::Read])
            .map(|entry| {
                entry
                    .allowed_ops
                    .iter()
                    .filter_map(|operation| FsOperation::parse(operation))
                    .collect()
            })
            .unwrap_or_default()
    }
}

pub(super) struct RecentView;
impl VfsProvider for RecentView {
    fn operations(&self) -> &'static [FsOperation] {
        &[
            FsOperation::List,
            FsOperation::Stat,
            FsOperation::Read,
            FsOperation::Search,
        ]
    }

    fn discover(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<FsOperation>, ProviderError> {
        Ok(recent_operations(path(ctx)?))
    }

    fn search_capabilities(&self, ctx: &dyn ProviderInvocation) -> SearchCapabilities {
        SearchCapabilities {
            name: path(ctx)
                .is_ok_and(|path| recent_operations(path).contains(&FsOperation::Search)),
            content: false,
        }
    }

    fn stat(&self, request: &FsRequest<'_>) -> Result<FsEntry, ProviderError> {
        let path = path(request.ctx())?;
        storage::recent_stat(path, &recent_operations(path))
            .map(shared_entry)
            .map_err(provider_error)
    }

    fn list(&self, request: &FsRequest<'_>) -> Result<FsListing, ProviderError> {
        storage::recent_list(path(request.ctx())?)
            .map(list_result)
            .map_err(provider_error)
    }

    fn read(&self, request: &FsRequest<'_>) -> Result<FsRead, ProviderError> {
        let alias = request.node_id().unwrap_or(path(request.ctx())?);
        let source = crate::core::recent::source_for(alias).map_err(provider_error)?;
        let broker =
            crate::core::fs::trusted(&source, owner(request.ctx())?).map_err(provider_error)?;
        broker
            .require(FsOperation::Read, false)
            .map_err(provider_error)?;
        let size = broker
            .stat()
            .map_err(provider_error)?
            .get("size")
            .and_then(Value::as_u64);
        let handle = broker.open_read_handle().map_err(provider_error)?;
        Ok(FsRead::new(BrokerReader { handle, offset: 0 }, size))
    }

    fn search(&self, request: &FsRequest<'_>) -> Result<FsListing, ProviderError> {
        let root = path(request.ctx())?;
        search_listing(root, request, storage::recent_list)
    }
}
