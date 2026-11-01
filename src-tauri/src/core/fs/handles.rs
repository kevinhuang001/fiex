//! Resource stream handles around the shared VFS provider products.
use super::{FsError, FsErrorKind, FsOperation, ResourceBroker, CHUNK};
use fiex_provider_contracts::{FsRead, FsWrite};
use serde_json::Value;
use std::{
    io::{Read, Seek, SeekFrom, Write},
    sync::atomic::{AtomicU64, Ordering},
};

const MAX_SPOOL: u64 = 100 * 1024 * 1024 * 1024;
static RESERVED: AtomicU64 = AtomicU64::new(0);

pub trait ReadHandle: Send {
    fn size(&self) -> u64;
    fn read_at(&mut self, offset: u64, length: usize) -> Result<(Vec<u8>, bool), String>;
}

pub trait WriteHandle: Send {
    fn write(&mut self, bytes: &[u8]) -> Result<(), String>;
    fn seek(&mut self, kind: &str, offset: i64) -> Result<u64, String>;
    fn commit(self: Box<Self>) -> Result<Value, String>;
}

/// A sequential shared-provider reader with a bounded working buffer. Bytes
/// already consumed are retained in a private temporary file so Core's
/// random-offset stream API remains available for every provider.
pub struct ProviderReadHandle {
    reader: Box<dyn Read + Send>,
    spool: std::fs::File,
    read_at: Option<
        Box<
            dyn FnMut(u64, usize) -> Result<(Vec<u8>, bool), fiex_provider_contracts::ProviderError>
                + Send,
        >,
    >,
    size: u64,
    cached: u64,
    eof: bool,
}

impl ProviderReadHandle {
    pub fn new(mut read: FsRead) -> Result<Self, FsError> {
        let mut spool =
            tempfile::tempfile().map_err(|error| FsError::internal(error.to_string()))?;
        let mut cached = 0u64;
        let mut eof = false;
        if read.read_at.is_some() && read.size.is_none() {
            return Err(FsError::new(
                FsErrorKind::Unsupported,
                "random read requires a known size",
            ));
        }
        if read.size.is_none() && read.read_at.is_none() {
            let mut buffer = vec![0; CHUNK];
            loop {
                let count = read
                    .reader
                    .read(&mut buffer)
                    .map_err(|error| FsError::internal(error.to_string()))?;
                if count == 0 {
                    eof = true;
                    break;
                }
                spool
                    .write_all(&buffer[..count])
                    .map_err(|error| FsError::internal(error.to_string()))?;
                cached = cached
                    .checked_add(count as u64)
                    .ok_or_else(|| FsError::internal("read size overflow"))?;
            }
        }
        Ok(Self {
            reader: read.reader,
            spool,
            read_at: read.read_at,
            size: read.size.unwrap_or(cached),
            cached,
            eof,
        })
    }
}

impl ReadHandle for ProviderReadHandle {
    fn size(&self) -> u64 {
        self.size
    }

    fn read_at(&mut self, offset: u64, length: usize) -> Result<(Vec<u8>, bool), String> {
        if let Some(read_at) = self.read_at.as_mut() {
            let (bytes, eof) =
                read_at(offset, length.min(CHUNK)).map_err(|error| error.to_string())?;
            if bytes.len() > length.min(CHUNK) {
                return Err("provider exceeded chunk limit".into());
            }
            return Ok((bytes, eof));
        }
        if offset >= self.size || length == 0 {
            return Ok((Vec::new(), offset >= self.size));
        }
        let end = offset.saturating_add(length as u64).min(self.size);
        let mut buffer = vec![0; CHUNK];
        while self.cached < end && !self.eof {
            let want = CHUNK.min(usize::try_from(end - self.cached).unwrap_or(usize::MAX));
            let count = self
                .reader
                .read(&mut buffer[..want])
                .map_err(|error| error.to_string())?;
            if count == 0 {
                self.eof = true;
                break;
            }
            self.spool
                .seek(SeekFrom::End(0))
                .and_then(|_| self.spool.write_all(&buffer[..count]))
                .map_err(|error| error.to_string())?;
            self.cached = self
                .cached
                .checked_add(count as u64)
                .ok_or("read offset overflow")?;
        }
        self.spool
            .seek(SeekFrom::Start(offset))
            .map_err(|error| error.to_string())?;
        let mut bytes = vec![0; length.min(CHUNK)];
        let count = self
            .spool
            .read(&mut bytes)
            .map_err(|error| error.to_string())?;
        bytes.truncate(count);
        let eof = offset.saturating_add(count as u64) >= self.size
            || (self.eof && self.cached <= offset.saturating_add(count as u64));
        Ok((bytes, eof))
    }
}

/// Direct write stream used by transfer and rollback operations. Shared VFS
/// writes are sequential, so seeking is limited to the current position.
pub struct ProviderWriteHandle {
    provider: Option<FsWrite>,
    offset: u64,
}

impl ProviderWriteHandle {
    pub fn new(provider: FsWrite) -> Self {
        Self {
            provider: Some(provider),
            offset: 0,
        }
    }
}

impl WriteHandle for ProviderWriteHandle {
    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        let provider = self.provider.as_mut().ok_or("write stream closed")?;
        provider
            .writer
            .write_all(bytes)
            .map_err(|error| error.to_string())?;
        self.offset = self
            .offset
            .checked_add(bytes.len() as u64)
            .ok_or("write offset overflow")?;
        Ok(())
    }

    fn seek(&mut self, kind: &str, offset: i64) -> Result<u64, String> {
        let requested = match kind {
            "current" if offset == 0 => self.offset,
            "start" if u64::try_from(offset).ok() == Some(self.offset) => self.offset,
            _ => return Err("Unsupported: provider writes are sequential".into()),
        };
        Ok(requested)
    }

    fn commit(mut self: Box<Self>) -> Result<Value, String> {
        let provider = self.provider.take().ok_or("write stream closed")?;
        let entry = provider.commit().map_err(|error| error.to_string())?;
        let _ = entry;
        Ok(Value::Null)
    }
}

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
