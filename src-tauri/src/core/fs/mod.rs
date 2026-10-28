//! Core-owned file access. Resource identities are resolved only inside an
//! operation scope; adapters never receive plugin-supplied filesystem paths.
mod binding;
mod builtin;
mod export;
mod presentation;
mod transfer;
pub(crate) use binding::trusted;
pub use transfer::transfer;
mod local;
pub use export::export as export_for_external_drag;
mod plugin;
mod streams;
mod usb;
use crate::plugins::provider_registry::FsOperation;
use crate::plugins::{contracts::ResourceRef, resource::fs_access::Scope};
use serde_json::Value;
pub use streams::{cleanup, http_body, stream_call, stream_permission};

pub const CHUNK: usize = 256 * 1024;
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
    fn stat(&self) -> Result<Value, String>;
    fn desktop_entry(&self) -> Result<crate::core::files::Entry, String> {
        presentation::entry(self.stat()?)
    }
    fn desktop_listing(
        &self,
        path: &str,
        hidden: bool,
        ignores: &[String],
    ) -> Result<crate::core::files::Listing, String> {
        presentation::listing(self, path, hidden, ignores)
    }
    fn list(&self) -> Result<Value, String>;
    fn require(&self, operation: FsOperation, create: bool) -> Result<(), String>;
    fn open_read(&self) -> Result<Box<dyn ReadHandle>, String>;
    fn begin_write(&self, create: bool) -> Result<Box<dyn WriteHandle>, String>;
    fn mkdir(&self) -> Result<Value, String>;
    fn remove(&self) -> Result<(), String>;
    fn rename(&self, destination: &ResourceBroker) -> Result<(), String>;
}

/// This value can only be constructed from a ref captured by Core's Scope.
pub struct ResourceBroker {
    pub reference: ResourceRef,
    pub(crate) path: String,
    pub(crate) scope: Scope,
    pub(crate) owner: String,
}
impl ResourceBroker {
    /// Desktop preview reads only the requested prefix, regardless of which
    /// storage adapter owns the resource.
    pub fn preview_prefix(&self, limit: usize) -> Result<Vec<u8>, String> {
        self.require(FsOperation::Read, false)?;
        let mut handle = self.adapter()?.open_read()?;
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
    pub fn resolve(scope: &Scope, owner: &str, reference: &Value) -> Result<Self, String> {
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
    fn adapter(&self) -> Result<Box<dyn FsAdapter + '_>, String> {
        if crate::plugins::mounts::handles(&self.path) {
            return Ok(Box::new(plugin::PluginFsAdapter::new(self)?));
        }
        if let Some(route) = crate::usbfs::route(&self.path) {
            return Ok(Box::new(usb::UsbFsAdapter::new(route)));
        }
        // Core-owned views are in-process storage adapters, never host paths.
        if crate::core::volumes::virtual_resolvers::resolver_for(&self.path).is_some() {
            return Ok(Box::new(builtin::BuiltinFsAdapter(self)));
        }
        self.scope.authorize(&self.path)?;
        Ok(Box::new(local::LocalFsAdapter(self)))
    }
    pub fn require(&self, operation: FsOperation, create: bool) -> Result<(), String> {
        if crate::usbfs::route(&self.path).is_some() {
            let current =
                crate::plugins::resource::resource_context::ResourceSnapshot::from_metadata(
                    &self.path,
                    Default::default(),
                );
            if current.reference != self.reference {
                return Err("StaleRef: USB mount generation changed".into());
            }
        }
        // Unix Trash is a physical directory, not an opaque URI. Do not let
        // its local filesystem adapter bypass the view policy: restore and
        // permanent deletion must remain explicit Core-only actions.
        if crate::core::volumes::virtual_resolvers::resolver_for(&self.path)
            .is_some_and(|resolver| resolver.id() == "core:trash")
            && !matches!(operation, FsOperation::Stat | FsOperation::List)
        {
            return Err("PermissionDenied: Trash requires a dedicated Core action".into());
        }
        self.adapter()?.require(operation, create)
    }
    pub fn stat(&self) -> Result<Value, String> {
        self.scope.authorize_method("fs.stat")?;
        self.require(FsOperation::Stat, false)?;
        let mut value = self.adapter()?.stat()?;
        // Internal stream/transfer DTOs use one symlink field across adapters.
        if let Some(symlink) = value.get("isSymlink").cloned() {
            value["symlink"] = symlink;
        }
        Ok(value)
    }
    pub fn list(&self) -> Result<Value, String> {
        self.scope.authorize_method("fs.list")?;
        self.require(FsOperation::List, false)?;
        self.adapter()?.list()
    }
    pub fn is_dir(&self) -> Result<bool, String> {
        Ok(self.stat()?["isDir"] == true)
    }
    pub fn open_read(&self) -> Result<Value, String> {
        self.scope.authorize_method("fs.openRead")?;
        self.require(FsOperation::Read, false)?;
        streams::open_read(self, self.adapter()?.open_read()?)
    }
    pub fn begin_write(&self, create: bool) -> Result<Value, String> {
        self.scope.authorize_method("fs.beginWrite")?;
        self.require(FsOperation::Write, create)?;
        streams::begin_write(self, self.adapter()?.begin_write(create)?, create)
    }
    pub fn mkdir(&self) -> Result<Value, String> {
        self.scope.authorize_method("fs.mkdir")?;
        self.require(FsOperation::Mkdir, true)?;
        self.adapter()?.mkdir()
    }
    pub fn remove(&self) -> Result<(), String> {
        self.scope.authorize_method("fs.remove")?;
        self.require(FsOperation::Remove, false)?;
        self.adapter()?.remove()
    }
    pub fn rename(&self, destination: &Self) -> Result<(), String> {
        self.scope.authorize_method("fs.rename")?;
        self.require(FsOperation::Rename, false)?;
        destination.require(FsOperation::Write, true)?;
        if self.reference.mount_id != destination.reference.mount_id {
            return Err("Unsupported: cross-mount rename requires TransferEngine".into());
        }
        self.adapter()?.rename(destination)
    }
    pub(crate) fn clone_binding(&self) -> Self {
        Self {
            reference: self.reference.clone(),
            path: self.path.clone(),
            scope: self.scope.clone(),
            owner: self.owner.clone(),
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod migration_tests;
