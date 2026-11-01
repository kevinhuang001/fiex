//! Storage adapter contract shared by all backends.
use super::presentation;
use super::{FsError, FsErrorKind, FsOperation, ResourceBroker};
use serde_json::Value;
pub trait ReadHandle: Send {
    fn size(&self) -> u64;
    fn read_at(&mut self, offset: u64, length: usize) -> Result<(Vec<u8>, bool), String>;
}
pub trait WriteHandle: Send {
    fn write(&mut self, bytes: &[u8]) -> Result<(), String>;
    fn seek(&mut self, kind: &str, offset: i64) -> Result<u64, String>;
    fn commit(self: Box<Self>) -> Result<Value, String>;
}
/// Storage only: presentation, menus and launch/restore actions stay outside.
pub trait FsAdapter: Send {
    /// Only an adapter with a real OS file may bypass staging. Virtual views,
    /// including views of local bytes, keep their own export policy.
    fn native_export_path(&self) -> Option<std::path::PathBuf> {
        None
    }
    fn preflight(
        &self,
        _request: &fiex_provider_contracts::TransferPreflight,
    ) -> Result<(), FsError> {
        Ok(())
    }
    fn stat(&self) -> Result<Value, FsError>;
    fn desktop_entry(&self) -> Result<crate::core::files::Entry, FsError> {
        presentation::entry(self.stat()?).map_err(FsError::from)
    }
    fn desktop_listing(
        &self,
        path: &str,
        hidden: bool,
        ignores: &[String],
    ) -> Result<crate::core::files::Listing, FsError> {
        presentation::listing(self, path, hidden, ignores).map_err(FsError::from)
    }
    fn list(&self) -> Result<Value, FsError>;
    fn require(&self, operation: FsOperation, create: bool) -> Result<(), FsError>;
    fn open_read(&self) -> Result<Box<dyn ReadHandle>, FsError>;
    fn begin_write(&self, create: bool) -> Result<Box<dyn WriteHandle>, FsError>;
    fn begin_write_sized(
        &self,
        create: bool,
        _size: Option<u64>,
    ) -> Result<Box<dyn WriteHandle>, FsError> {
        self.begin_write(create)
    }
    fn mkdir(&self) -> Result<Value, FsError>;
    fn search(
        &self,
        _query: &str,
        _hidden: bool,
        _ignores: &[String],
        _limit: usize,
        _cancelled: &dyn Fn() -> bool,
    ) -> Result<(Vec<crate::core::files::Entry>, bool, usize), FsError> {
        Err(FsError::new(
            FsErrorKind::Unsupported,
            "search is not implemented by this adapter",
        ))
    }
    #[allow(clippy::too_many_arguments)]
    fn search_stream(
        &self,
        query: &str,
        hidden: bool,
        ignores: &[String],
        limit: usize,
        cancelled: &dyn Fn() -> bool,
        emit: &mut dyn FnMut(crate::core::files::Entry),
    ) -> Result<(bool, usize), FsError> {
        let (entries, truncated, skipped) =
            self.search(query, hidden, ignores, limit, cancelled)?;
        for entry in entries {
            if cancelled() {
                break;
            }
            emit(entry);
        }
        Ok((truncated, skipped))
    }
    fn remove(&self) -> Result<(), FsError>;
    /// Removing a node through the desktop action keeps its storage semantics.
    /// Only the local adapter may hand the node to the OS Trash; every other
    /// adapter performs the same remove its `remove` operation already owns.
    fn move_to_trash(&self) -> Result<(), FsError> {
        self.remove()
    }
    fn rename(&self, destination: &ResourceBroker) -> Result<(), FsError>;
}
