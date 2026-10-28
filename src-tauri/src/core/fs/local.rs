use super::*;
use crate::plugins::resource::fs_access;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::json;

pub(super) struct LocalFsAdapter<'a>(pub &'a ResourceBroker);
impl FsAdapter for LocalFsAdapter<'_> {
    fn desktop_entry(&self) -> Result<crate::core::files::Entry, String> {
        self.0.scope.authorize(&self.0.path)?;
        crate::core::files::entry(std::path::Path::new(&self.0.path))
    }
    fn desktop_listing(
        &self,
        path: &str,
        hidden: bool,
        ignores: &[String],
    ) -> Result<crate::core::files::Listing, String> {
        self.0.scope.authorize(path)?;
        crate::core::files::list(path, hidden, ignores)
    }
    fn stat(&self) -> Result<Value, String> {
        fs_access::stat(&self.0.scope, &self.0.path)
    }
    fn list(&self) -> Result<Value, String> {
        fs_access::list(&self.0.scope, &self.0.path)
    }
    fn require(&self, op: FsOperation, create: bool) -> Result<(), String> {
        self.0.scope.authorize(&self.0.path)?;
        // Creation targets may not exist. Validate their scoped parent instead.
        if create || op == FsOperation::Mkdir {
            std::path::Path::new(&self.0.path)
                .parent()
                .ok_or("invalid parent")?;
            return Ok(()); // cap-std and the OS enforce the directory ACL at execution.
        }
        self.stat()?;
        Ok(()) // File READONLY is a display attribute, not rename/delete permission.
    }
    fn open_read(&self) -> Result<Box<dyn ReadHandle>, String> {
        let v = fs_access::open_read(&self.0.owner, &self.0.scope, &self.0.path)?;
        Ok(Box::new(LocalRead {
            owner: self.0.owner.clone(),
            handle: v["handle"].as_str().ok_or("missing read handle")?.into(),
            size: v["size"].as_u64().ok_or("missing size")?,
        }))
    }
    fn begin_write(&self, create: bool) -> Result<Box<dyn WriteHandle>, String> {
        let handle = fs_access::begin_write(&self.0.owner, &self.0.scope, &self.0.path, create)?;
        Ok(Box::new(LocalWrite {
            owner: self.0.owner.clone(),
            handle,
            finished: false,
        }))
    }
    fn mkdir(&self) -> Result<Value, String> {
        fs_access::mkdir(&self.0.scope, &self.0.path)
    }
    fn remove(&self) -> Result<(), String> {
        fs_access::remove(&self.0.scope, &self.0.path)
    }
    fn rename(&self, destination: &ResourceBroker) -> Result<(), String> {
        fs_access::rename(&self.0.scope, &self.0.path, &destination.path)
    }
}
struct LocalRead {
    owner: String,
    handle: String,
    size: u64,
}
impl ReadHandle for LocalRead {
    fn size(&self) -> u64 {
        self.size
    }
    fn read_at(&mut self, offset: u64, length: usize) -> Result<(Vec<u8>, bool), String> {
        let v = fs_access::read_open(&self.owner, &self.handle, offset, length.min(CHUNK), None)?;
        let bytes = STANDARD
            .decode(v["base64"].as_str().ok_or("missing data")?)
            .map_err(|e| e.to_string())?;
        Ok((bytes, v["eof"] == true))
    }
}
impl Drop for LocalRead {
    fn drop(&mut self) {
        let _ = fs_access::close_read(&self.owner, &self.handle);
    }
}
struct LocalWrite {
    owner: String,
    handle: String,
    finished: bool,
}
impl WriteHandle for LocalWrite {
    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        fs_access::write_chunk(&self.owner, &self.handle, &STANDARD.encode(bytes))
    }
    fn seek(&mut self, kind: &str, offset: i64) -> Result<u64, String> {
        fs_access::seek_write(&self.owner, &self.handle, kind, offset)
    }
    fn commit(mut self: Box<Self>) -> Result<Value, String> {
        let result = fs_access::commit(&self.owner, &self.handle)?;
        self.finished = true;
        // Public file services never return the internal path of a committed file.
        let _ = result;
        Ok(json!(null))
    }
}
impl Drop for LocalWrite {
    fn drop(&mut self) {
        if !self.finished {
            let _ = fs_access::abort(&self.owner, &self.handle);
        }
    }
}
