//! Unknown-length output is spooled privately; providers receive a known
//! size at commit. Transfers with a known source length stream directly.
use super::*;
use std::io::{Read, Seek, SeekFrom, Write};
const MAX_SPOOL: u64 = 100 * 1024 * 1024 * 1024;
use std::sync::atomic::{AtomicU64, Ordering};
static RESERVED: AtomicU64 = AtomicU64::new(0);
pub(super) struct DeferredWrite {
    resource: ResourceBroker,
    create: bool,
    file: std::fs::File,
    original: Option<Value>,
    reserved: u64,
}
impl DeferredWrite {
    pub fn new(resource: &ResourceBroker, create: bool) -> Result<Self, String> {
        Ok(Self {
            resource: resource.clone_binding(),
            create,
            file: tempfile::tempfile().map_err(|e| e.to_string())?,
            original: if create { None } else { Some(resource.stat()?) },
            reserved: 0,
        })
    }
}
impl WriteHandle for DeferredWrite {
    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        let position = self.file.stream_position().map_err(|e| e.to_string())?;
        if position
            .checked_add(bytes.len() as u64)
            .is_none_or(|end| end > MAX_SPOOL)
        {
            return Err("write spool quota exceeded".into());
        }
        let end = position + bytes.len() as u64;
        let additional = end.saturating_sub(self.reserved);
        RESERVED
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |used| {
                used.checked_add(additional)
                    .filter(|total| *total <= MAX_SPOOL)
            })
            .map_err(|_| "aggregate write spool quota exceeded")?;
        self.reserved += additional;
        self.file.write_all(bytes).map_err(|e| e.to_string())
    }
    fn seek(&mut self, kind: &str, offset: i64) -> Result<u64, String> {
        let from = match kind {
            "start" => SeekFrom::Start(u64::try_from(offset).map_err(|_| "negative offset")?),
            "current" => SeekFrom::Current(offset),
            "end" => SeekFrom::End(offset),
            _ => return Err("invalid seek kind".into()),
        };
        let position = self.file.seek(from).map_err(|e| e.to_string())?;
        if position > MAX_SPOOL {
            return Err("write spool quota exceeded".into());
        }
        Ok(position)
    }
    fn commit(mut self: Box<Self>) -> Result<Value, String> {
        self.resource.require(FsOperation::Write, self.create)?;
        let adapter = PluginFsAdapter::new(&self.resource)?;
        let current = adapter.stat();
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
        let size = self.file.metadata().map_err(|e| e.to_string())?.len();
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|e| e.to_string())?;
        let mut writer = adapter.begin_write_sized(self.create, Some(size))?;
        let mut buffer = vec![0; CHUNK];
        loop {
            let length = self.file.read(&mut buffer).map_err(|e| e.to_string())?;
            if length == 0 {
                break;
            }
            writer.write(&buffer[..length])?;
        }
        writer.commit()
    }
}
impl Drop for DeferredWrite {
    fn drop(&mut self) {
        RESERVED.fetch_sub(self.reserved, Ordering::SeqCst);
    }
}
pub(super) struct PluginWrite {
    resource: ResourceBroker,
    stream: Option<FsWriteStream>,
    offset: u64,
}
impl PluginWrite {
    pub fn new(resource: &ResourceBroker, stream: FsWriteStream) -> Self {
        Self {
            resource: resource.clone_binding(),
            stream: Some(stream),
            offset: 0,
        }
    }
}
impl WriteHandle for PluginWrite {
    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.stream
            .as_mut()
            .ok_or("write handle closed")?
            .write(bytes)?;
        self.offset = self
            .offset
            .checked_add(bytes.len() as u64)
            .ok_or("write offset overflow")?;
        Ok(())
    }
    fn seek(&mut self, kind: &str, offset: i64) -> Result<u64, String> {
        if (kind == "current" && offset == 0)
            || (kind == "start" && u64::try_from(offset).ok() == Some(self.offset))
        {
            return Ok(self.offset);
        }
        Err("Unsupported: provider writes are sequential".into())
    }
    fn commit(mut self: Box<Self>) -> Result<Value, String> {
        if let Some(entry) = self.stream.take().ok_or("write handle closed")?.commit()? {
            PluginFsAdapter::new(&self.resource)?.remember(&entry)?;
        }
        Ok(Value::Null)
    }
}
