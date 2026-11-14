//! Write and mutation operations for scoped filesystem resources.

use super::ResourceBroker;
use crate::core::fs::{
    FsError, WriteHandle, binding, changes, parent_path, provider_registry, streams,
};
use fiex_provider_contracts::FsOperation;
use serde_json::Value;

impl ResourceBroker {
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
        if !directory {
            return self.create_file_with_bytes(name, &[]);
        }
        self.scope.authorize_method("host.fs.mkdir")?;
        let child = binding::child(self, name, provider_registry::ChildKind::Folder)?;
        child.mkdir()?;
        Ok(child.path)
    }
    /// Create a new file and commit its initial bytes through the same
    /// provider-owned write stream used for ordinary file creation.
    pub fn create_file_with_bytes(&self, name: &str, bytes: &[u8]) -> Result<String, FsError> {
        self.scope.authorize_method("host.fs.beginWrite")?;
        let child = binding::child(self, name, provider_registry::ChildKind::File)?;
        // The child's create-write check binds the parent's child-create
        // capability; checking this broker would grant from its parent.
        child.require(FsOperation::Write, true)?;
        let mut writer = child.begin_write_handle_sized(true, Some(bytes.len() as u64))?;
        writer.write(bytes)?;
        writer.commit()?;
        changes::created(&child.path, name, false);
        Ok(child.path)
    }
    /// Move this node to the platform Trash when local storage owns it.
    pub fn move_to_trash(&self) -> Result<(), FsError> {
        self.scope.authorize_method("host.fs.trash")?;
        self.require(FsOperation::Trash, false)?;
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
        let parent = parent_path(&self.path).ok();
        provider_registry::remove(self)?;
        if let Some(parent) = parent {
            changes::removed_from(&self.path, &parent);
        } else {
            changes::removed(&self.path);
        }
        Ok(())
    }
    pub fn rename(&self, destination: &Self) -> Result<String, FsError> {
        self.scope.authorize_method("host.fs.rename")?;
        self.require(FsOperation::Rename, false)?;
        destination.require(FsOperation::Write, true)?;
        if self.reference.mount_id != destination.reference.mount_id {
            return Err("Unsupported: cross-mount rename requires TransferEngine".into());
        }
        let path = provider_registry::rename(self, destination)?;
        changes::renamed(&self.path, &path);
        Ok(path)
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
