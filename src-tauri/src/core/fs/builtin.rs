//! Core-owned virtual storage adapter; never turns a view URI into a host path.
use super::*;
use crate::core::{recent, volumes::{applications, virtual_resolvers}};
use serde_json::{json, to_value};
use std::{fs, io::{Read, Seek, SeekFrom, Write}, path::PathBuf};

pub(super) struct BuiltinFsAdapter<'a>(pub &'a ResourceBroker);
impl BuiltinFsAdapter<'_> {
    fn path(&self) -> &str { &self.0.path }
    fn application_file(&self) -> Result<PathBuf, String> {
        if !applications::is_user(self.path()) {
            return Err("Unsupported: system application has no file stream".into());
        }
        applications::candidate_path(self.path())
    }
    fn application_capability(&self, op: FsOperation, create: bool) -> Result<(), String> {
        let path = self.application_file()?;
        let exists = path.exists();
        let directory = path.is_dir();
        if create {
            if !matches!(op, FsOperation::Write | FsOperation::Mkdir) {
                return Err("Unsupported: invalid application create operation".into());
            }
            let parent = path.parent().ok_or("missing application parent")?;
            if !parent.is_dir() || exists {
                return Err("Conflict: application destination is unavailable".into());
            }
            return Ok(());
        }
        if !exists { return Err("NotFound: application item missing".into()); }
        let allowed = match op {
            FsOperation::Stat => true,
            FsOperation::List | FsOperation::Search | FsOperation::Mkdir => directory,
            FsOperation::Read | FsOperation::Write => !directory,
            FsOperation::Rename | FsOperation::Remove => true,
        };
        if allowed { Ok(()) } else { Err(format!("Unsupported: {}", op.as_str())) }
    }
}
impl FsAdapter for BuiltinFsAdapter<'_> {
    fn stat(&self) -> Result<Value, String> {
        to_value(virtual_resolvers::resource_entry(self.path())?).map_err(|e| e.to_string())
    }
    fn list(&self) -> Result<Value, String> {
        let items = if self.path() == virtual_resolvers::RECENT_URI {
            recent::entries(&recent::Recent::default())
        } else {
            let resolver = virtual_resolvers::resolver_for(self.path())
                .ok_or("NotFound: builtin view missing")?;
            resolver.list(self.path(), &virtual_resolvers::VirtualContext { recent: &[] })?
        };
        to_value(items).map_err(|e| e.to_string())
    }
    fn require(&self, op: FsOperation, create: bool) -> Result<(), String> {
        if self.path() == applications::URI {
            return if matches!(op, FsOperation::Stat | FsOperation::List) {
                Ok(())
            } else { Err("Unsupported: applications root has no file stream".into()) };
        }
        if applications::is_user(self.path()) {
            return self.application_capability(op, create);
        }
        if virtual_resolvers::resolver_for(self.path())
            .is_some_and(|resolver| resolver.id() == "core:applications")
        {
            return if op == FsOperation::Stat { Ok(()) } else {
                Err("Unsupported: system application is launch-only".into())
            };
        }
        if virtual_resolvers::resolver_for(self.path())
            .is_some_and(|resolver| resolver.id() == "core:trash")
        {
            return if op == FsOperation::Stat
                || (op == FsOperation::List && crate::core::volumes::trash::trash_path()
                    .is_some_and(|root| root.to_string_lossy() == self.path()))
                || (op == FsOperation::List && self.path() == crate::core::volumes::recycle::URI)
                { Ok(()) } else {
                    Err("PermissionDenied: Trash requires a dedicated Core action".into())
                };
        }
        if self.path() == virtual_resolvers::RECENT_URI
            && matches!(op, FsOperation::List | FsOperation::Stat) { return Ok(()); }
        if self.path().starts_with(virtual_resolvers::RECENT_URI) {
            let source = recent::source_for(self.path())?;
            if op == FsOperation::Stat { return Ok(()); }
            if op == FsOperation::Read {
                let original = super::binding::trusted(&source, &self.0.owner)?;
                return original.require(FsOperation::Read, false);
            }
            return Err("PermissionDenied: Recent is a read-only view".into());
        }
        Err(format!("Unsupported: {}", op.as_str()))
    }
    fn open_read(&self) -> Result<Box<dyn ReadHandle>, String> {
        self.require(FsOperation::Read, false)?;
        if self.path().starts_with(virtual_resolvers::RECENT_URI) {
            let source = recent::source_for(self.path())?;
            let original = super::binding::trusted(&source, &self.0.owner)?;
            original.require(FsOperation::Read, false)?;
            return original.adapter()?.open_read();
        }
        let file = fs::File::open(self.application_file()?).map_err(|e| e.to_string())?;
        let size = file.metadata().map_err(|e| e.to_string())?.len();
        Ok(Box::new(AppRead { file, size }))
    }
    fn begin_write(&self, create: bool) -> Result<Box<dyn WriteHandle>, String> {
        self.require(FsOperation::Write, create)?;
        let path = self.application_file()?;
        let temporary = tempfile::NamedTempFile::new_in(path.parent().ok_or("missing parent")?)
            .map_err(|e| e.to_string())?;
        Ok(Box::new(AppWrite { temporary: Some(temporary), target: path, create }))
    }
    fn mkdir(&self) -> Result<Value, String> {
        self.require(FsOperation::Mkdir, true)?;
        fs::create_dir(self.application_file()?).map_err(|e| e.to_string())?;
        self.stat()
    }
    fn remove(&self) -> Result<(), String> {
        self.require(FsOperation::Remove, false)?;
        applications::remove_user(self.path())
    }
    fn rename(&self, destination: &ResourceBroker) -> Result<(), String> {
        self.require(FsOperation::Rename, false)?;
        if !applications::is_user(&destination.path) {
            return Err("ScopeDenied: cannot rename into another view".into());
        }
        let from = self.application_file()?;
        let to = applications::candidate_path(&destination.path)?;
        if from.parent() != to.parent() {
            return Err("Unsupported: moving application items requires TransferEngine".into());
        }
        if to.exists() { return Err("Conflict: application destination exists".into()); }
        fs::rename(from, to).map_err(|e| e.to_string())
    }
}
struct AppRead { file: fs::File, size: u64 }
impl ReadHandle for AppRead {
    fn size(&self) -> u64 { self.size }
    fn read_at(&mut self, offset: u64, length: usize) -> Result<(Vec<u8>, bool), String> {
        self.file.seek(SeekFrom::Start(offset)).map_err(|e| e.to_string())?;
        let mut data = vec![0; length.min(CHUNK)];
        let n = self.file.read(&mut data).map_err(|e| e.to_string())?;
        data.truncate(n);
        Ok((data, offset.saturating_add(n as u64) >= self.size))
    }
}
struct AppWrite { temporary: Option<tempfile::NamedTempFile>, target: PathBuf, create: bool }
impl WriteHandle for AppWrite {
    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.temporary.as_mut().ok_or("stream closed")?.as_file_mut()
            .write_all(bytes).map_err(|e| e.to_string())
    }
    fn seek(&mut self, kind: &str, offset: i64) -> Result<u64, String> {
        let from = match kind {
            "start" => SeekFrom::Start(u64::try_from(offset).map_err(|e| e.to_string())?),
            "current" => SeekFrom::Current(offset),
            "end" => SeekFrom::End(offset),
            _ => return Err("Unsupported: invalid seek operation".into()),
        };
        self.temporary.as_mut().ok_or("stream closed")?.as_file_mut()
            .seek(from).map_err(|e| e.to_string())
    }
    fn commit(mut self: Box<Self>) -> Result<Value, String> {
        let temp = self.temporary.take().ok_or("stream closed")?;
        temp.as_file().sync_all().map_err(|e| e.to_string())?;
        if self.create {
            temp.persist_noclobber(&self.target).map_err(|e| e.error.to_string())?;
        } else {
            temp.persist(&self.target).map_err(|e| e.error.to_string())?;
        }
        Ok(json!(null))
    }
}
