//! Application-owned staged write streams around a scoped resource.
use super::{FsError, FsOperation, ResourceBroker};
use fiex_fs::{CHUNK, WriteHandle};
use serde_json::Value;
use std::{
    io::{Read, Seek, SeekFrom, Write},
    sync::atomic::{AtomicU64, Ordering},
};

pub use fiex_fs::{ProviderReadHandle, ProviderWriteHandle};

const MAX_SPOOL: u64 = 100 * 1024 * 1024 * 1024;
static RESERVED: AtomicU64 = AtomicU64::new(0);

/// Public `host.fs.beginWrite` supports seeks; providers receive a sized sequential
/// stream only after Core has staged and validated the complete output.
pub struct StagedWriteHandle {
    resource: ResourceBroker,
    create: bool,
    file: tempfile::NamedTempFile,
    original: Option<Value>,
    reserved: u64,
}

impl StagedWriteHandle {
    pub fn new(resource: &ResourceBroker, create: bool) -> Result<Self, FsError> {
        Ok(Self {
            resource: resource.clone_binding(),
            create,
            file: tempfile::NamedTempFile::new()
                .map_err(|error| FsError::internal(error.to_string()))?,
            original: if create { None } else { Some(resource.stat()?) },
            reserved: 0,
        })
    }
}

impl WriteHandle for StagedWriteHandle {
    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        let position = self
            .file
            .as_file_mut()
            .stream_position()
            .map_err(|error| error.to_string())?;
        let end = position
            .checked_add(bytes.len() as u64)
            .filter(|end| *end <= MAX_SPOOL)
            .ok_or("write spool quota exceeded")?;
        let additional = end.saturating_sub(self.reserved);
        let mut used = RESERVED.load(Ordering::Relaxed);
        loop {
            let next = used
                .checked_add(additional)
                .filter(|total| *total <= MAX_SPOOL)
                .ok_or("aggregate write spool quota exceeded")?;
            match RESERVED.compare_exchange_weak(used, next, Ordering::SeqCst, Ordering::Relaxed) {
                Ok(_) => break,
                Err(current) => used = current,
            }
        }
        self.reserved += additional;
        self.file
            .as_file_mut()
            .write_all(bytes)
            .map_err(|error| error.to_string())
    }

    fn seek(&mut self, kind: &str, offset: i64) -> Result<u64, String> {
        let from = match kind {
            "start" => SeekFrom::Start(u64::try_from(offset).map_err(|_| "negative offset")?),
            "current" => SeekFrom::Current(offset),
            "end" => SeekFrom::End(offset),
            _ => return Err("invalid seek kind".into()),
        };
        let position = self
            .file
            .as_file_mut()
            .seek(from)
            .map_err(|error| error.to_string())?;
        if position > MAX_SPOOL {
            return Err("write spool quota exceeded".into());
        }
        Ok(position)
    }

    fn commit(mut self: Box<Self>) -> Result<Value, String> {
        self.resource.require(FsOperation::Write, self.create)?;
        let current = self.resource.stat();
        if let Some(expected) = &self.original {
            let current = current?;
            if ["size", "modified", "revision", "isDir"]
                .iter()
                .any(|key| expected[*key] != current[*key])
            {
                return Err("Conflict: destination changed while output was staged".into());
            }
        } else if current.is_ok() {
            return Err("Conflict: destination appeared while output was staged".into());
        }
        let size = self
            .file
            .as_file()
            .metadata()
            .map_err(|error| error.to_string())?
            .len();
        self.file
            .as_file_mut()
            .seek(SeekFrom::Start(0))
            .map_err(|error| error.to_string())?;
        let mut provider =
            super::provider_registry::write_stream(&self.resource, self.create, Some(size))
                .map_err(String::from)?;
        let mut buffer = vec![0; CHUNK];
        loop {
            let count = self
                .file
                .as_file_mut()
                .read(&mut buffer)
                .map_err(|error| error.to_string())?;
            if count == 0 {
                break;
            }
            provider
                .writer
                .write_all(&buffer[..count])
                .map_err(|error| error.to_string())?;
        }
        provider.commit().map_err(|error| error.to_string())?;
        Ok(Value::Null)
    }
}

impl Drop for StagedWriteHandle {
    fn drop(&mut self) {
        RESERVED.fetch_sub(self.reserved, Ordering::SeqCst);
    }
}
