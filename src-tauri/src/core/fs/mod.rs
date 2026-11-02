//! Core-owned file access. Resource identities are resolved only inside an
//! operation scope; Providers never receive plugin-supplied filesystem paths.
mod binding;
mod cache_identity;
pub(crate) mod changes;
pub(crate) mod decoration_cache;
pub(crate) mod directory_cache;
mod error;
mod export;
mod list_service;
mod mutation_overlay;
mod preflight;
pub(crate) mod presentation;
mod transfer;
pub(crate) use binding::{child, parent_path, trusted};
#[cfg(feature = "desktop")]
pub(crate) use binding::{create, move_to_trash, remove_path, rename_path};
pub use error::{FsError, FsErrorKind};
pub use export::{export as export_for_external_drag, initialize as initialize_exports};
pub use transfer::transfer;
pub(crate) mod provider_registry;
mod streams;
use crate::plugins::provider_registry::FsOperation;
use crate::plugins::{contracts::ResourceRef, resource::fs_access::Scope};
use serde_json::Value;
pub use streams::{cleanup, http_body, stream_call, stream_permission};

pub const CHUNK: usize = 256 * 1024;
mod handles;
pub use handles::{ReadHandle, WriteHandle};

/// This value can only be constructed from a ref captured by Core's Scope.
pub struct ResourceBroker {
    pub reference: ResourceRef,
    pub(crate) path: String,
    pub(crate) scope: Scope,
    pub(crate) owner: String,
}
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
    pub fn resolve(scope: &Scope, owner: &str, reference: &Value) -> Result<Self, FsError> {
        if !scope.resource_bound(reference)? {
            return Err("ScopeDenied: resource not authorized in this operation".into());
        }
        let reference_typed = serde_json::from_value(reference.clone())
            .map_err(|e| format!("invalid ResourceRef: {e}"))?;
        let path = scope
            .remote_target(&serde_json::json!({"ref":reference}))?
            .map(|snapshot| snapshot.display)
            .map(Ok)
            .unwrap_or_else(|| scope.resolve_reference(reference))?;
        Ok(Self {
            reference: reference_typed,
            path,
            scope: scope.clone(),
            owner: owner.into(),
        })
    }
    pub(crate) fn open_read_handle(&self) -> Result<Box<dyn ReadHandle>, FsError> {
        self.require(FsOperation::Read, false)?;
        provider_registry::open_read(self)
    }
    pub fn require(&self, operation: FsOperation, create: bool) -> Result<(), FsError> {
        provider_registry::require(self, operation, create)
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
    pub fn begin_write(&self, create: bool) -> Result<Value, FsError> {
        self.scope.authorize_method("host.fs.beginWrite")?;
        self.require(FsOperation::Write, create)?;
        streams::begin_write(self, provider_registry::begin_write(self, create)?, create)
            .map_err(FsError::from)
    }
    /// Create one child in this directory through its owning provider. The
    /// parent's child-create capability is checked once, then the child uses
    /// the same scoped stream/mkdir path as any other storage write.
    pub fn create(&self, name: &str, directory: bool) -> Result<String, FsError> {
        self.scope.authorize_method(if directory {
            "host.fs.mkdir"
        } else {
            "host.fs.beginWrite"
        })?;
        let child = binding::child(self, name)?;
        if directory {
            child.mkdir()?;
        } else {
            // The child's create-write check binds the parent's child-create
            // capability; checking this broker's parent would grant from the
            // grandparent instead.
            child.require(FsOperation::Write, true)?;
            child.begin_write_handle_sized(true, Some(0))?.commit()?;
            changes::created(&child.path, name, false);
        }
        Ok(child.path)
    }
    /// Move this node to the platform Trash when local storage owns it.
    pub fn move_to_trash(&self) -> Result<(), FsError> {
        self.scope.authorize_method("host.fs.remove")?;
        self.require(FsOperation::Remove, false)?;
        provider_registry::move_to_trash(self)?;
        changes::removed(&self.path);
        Ok(())
    }
    pub fn mkdir(&self) -> Result<Value, FsError> {
        self.scope.authorize_method("host.fs.mkdir")?;
        self.require(FsOperation::Mkdir, true)?;
        let result = provider_registry::mkdir(self)?;
        changes::committed_result(&self.path, &result);
        Ok(result)
    }
    pub fn remove(&self) -> Result<(), FsError> {
        self.scope.authorize_method("host.fs.remove")?;
        self.require(FsOperation::Remove, false)?;
        provider_registry::remove(self)?;
        changes::removed(&self.path);
        Ok(())
    }
    pub fn rename(&self, destination: &Self) -> Result<(), FsError> {
        self.scope.authorize_method("host.fs.rename")?;
        self.require(FsOperation::Rename, false)?;
        destination.require(FsOperation::Write, true)?;
        if self.reference.mount_id != destination.reference.mount_id {
            return Err("Unsupported: cross-mount rename requires TransferEngine".into());
        }
        let metadata = self.transfer_metadata()?;
        let parent = binding::trusted(&binding::parent_path(&destination.path)?, &self.owner)?;
        use crate::plugin_sdk::filesystem::{TransferAction, TransferDirection};
        let name = destination
            .path
            .rsplit(['/', '\\'])
            .next()
            .ok_or("missing destination name")?;
        self.review_transfer(
            TransferDirection::Export,
            TransferAction::Move,
            name,
            &metadata,
            true,
        )?;
        parent.review_transfer(
            TransferDirection::Import,
            TransferAction::Move,
            name,
            &metadata,
            true,
        )?;
        provider_registry::rename(self, destination)?;
        changes::renamed(&self.path, &destination.path);
        Ok(())
    }
    pub(crate) fn clone_binding(&self) -> Self {
        Self {
            reference: self.reference.clone(),
            path: self.path.clone(),
            scope: self.scope.clone(),
            owner: self.owner.clone(),
        }
    }

    pub(crate) fn begin_write_handle_sized(
        &self,
        create: bool,
        size: Option<u64>,
    ) -> Result<Box<dyn WriteHandle>, FsError> {
        provider_registry::begin_write_sized(self, create, size)
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn begin_write_handle(&self, create: bool) -> Result<Box<dyn WriteHandle>, FsError> {
        provider_registry::begin_write(self, create)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod provider_tests;
