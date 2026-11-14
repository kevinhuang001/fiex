//! Read, list, and search operations for scoped filesystem resources.

use super::ResourceBroker;
use crate::core::fs::{
    CHUNK, FsError, FsErrorKind, ReadHandle, directory_cache, provider_registry, streams,
};
use fiex_provider_contracts::FsOperation;
use serde_json::Value;

impl ResourceBroker {
    /// Desktop preview reads only the requested prefix from the resource's
    /// registered VFS provider.
    pub fn preview_prefix(&self, limit: usize) -> Result<Vec<u8>, FsError> {
        self.require(FsOperation::Read, false)?;
        let mut handle = self.open_read_handle()?;
        let mut result = Vec::with_capacity(limit.min(CHUNK));
        while result.len() < limit {
            let (part, eof) =
                handle.read_at(result.len() as u64, (limit - result.len()).min(CHUNK))?;
            if part.is_empty() && !eof {
                return Err("source stream ended unexpectedly".into());
            }
            result.extend_from_slice(&part);
            if eof {
                break;
            }
        }
        Ok(result)
    }
    pub fn stat(&self) -> Result<Value, FsError> {
        self.scope.authorize_method("host.fs.stat")?;
        self.require(FsOperation::Stat, false)?;
        let mut value = provider_registry::stat(self)?;
        // Internal stream and transfer DTOs use one symlink field.
        if let Some(symlink) = value.get("isSymlink").cloned() {
            value["symlink"] = symlink;
        }
        Ok(value)
    }
    pub fn list(&self) -> Result<Value, FsError> {
        self.scope.authorize_method("host.fs.list")?;
        self.require(FsOperation::List, false)?;
        if directory_cache::enabled(&self.path) {
            let listing = self.desktop_listing(true, &[])?;
            return serde_json::to_value(listing.entries).map_err(|e| FsError::from(e.to_string()));
        }
        provider_registry::list(self)
    }
    pub fn is_dir(&self) -> Result<bool, FsError> {
        Ok(self.stat()?["isDir"] == true)
    }
    /// Run a bounded filename search through the node's owning VFS provider.
    pub fn search(
        &self,
        query: &str,
        hidden: bool,
        ignores: &[String],
        limit: usize,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(Vec<crate::core::files::Entry>, bool, usize), FsError> {
        self.scope.authorize_method("host.fs.search")?;
        self.require(FsOperation::Search, false)?;
        provider_registry::search(self, query, hidden, ignores, limit, cancelled)
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn search_stream(
        &self,
        query: &str,
        hidden: bool,
        ignores: &[String],
        limit: usize,
        cancelled: &dyn Fn() -> bool,
        emit: &mut dyn FnMut(crate::core::files::Entry),
    ) -> Result<(bool, usize), FsError> {
        self.scope.authorize_method("host.fs.search")?;
        match self.require(FsOperation::Search, false) {
            Ok(()) => {}
            Err(error) if error.kind == FsErrorKind::Unsupported => {
                self.require(FsOperation::List, false)?
            }
            Err(error) => return Err(error),
        }
        provider_registry::search_stream(self, query, hidden, ignores, limit, cancelled, emit)
    }
    pub fn open_read(&self) -> Result<Value, FsError> {
        self.scope.authorize_method("host.fs.openRead")?;
        self.require(FsOperation::Read, false)?;
        streams::open_read(self, self.open_read_handle()?).map_err(FsError::from)
    }
}
