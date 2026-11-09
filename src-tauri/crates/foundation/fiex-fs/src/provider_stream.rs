//! Adapters from provider-owned streams to the shared filesystem stream API.
use crate::{CHUNK, FsError, FsErrorKind, ReadHandle, WriteHandle};
use fiex_provider_contracts::{FsRead, FsReadAt, FsWrite};
use serde_json::Value;
use std::io::{Read, Seek, SeekFrom, Write};

/// A sequential shared-provider reader with a bounded working buffer. Bytes
/// already consumed are retained in a private temporary file so random-offset
/// reads remain available for every provider.
pub struct ProviderReadHandle {
    reader: Box<dyn Read + Send>,
    spool: std::fs::File,
    read_at: Option<FsReadAt>,
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

/// Direct write stream used by provider-to-provider transfer operations.
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
        provider.commit().map_err(|error| error.to_string())?;
        Ok(Value::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn sequential_reads_support_random_offsets_and_report_eof() {
        let mut handle = ProviderReadHandle::new(FsRead::new(Cursor::new(b"abcdef"), Some(6)))
            .expect("reader should initialize");
        assert_eq!(handle.read_at(2, 3).unwrap(), (b"cde".to_vec(), false));
        assert_eq!(handle.read_at(5, 3).unwrap(), (b"f".to_vec(), true));
    }

    #[test]
    fn unknown_size_sequential_stream_is_spooled_and_measured() {
        let mut handle = ProviderReadHandle::new(FsRead::new(Cursor::new(b"abc"), None))
            .expect("reader should initialize");
        assert_eq!(handle.size(), 3);
        assert_eq!(handle.read_at(0, 3).unwrap(), (b"abc".to_vec(), true));
    }

    #[test]
    fn random_access_provider_reads_must_declare_a_size() {
        let read = FsRead::with_read_at(Cursor::new([]), None, |_, _| Ok((Vec::new(), true)));
        let error = match ProviderReadHandle::new(read) {
            Ok(_) => panic!("random reads without a known size must be rejected"),
            Err(error) => error,
        };
        assert_eq!(error.kind, FsErrorKind::Unsupported);
    }

    #[test]
    fn writes_are_sequential_and_commit_provider_output() {
        let output = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let writer = output.clone();
        let mut handle = ProviderWriteHandle::new(FsWrite::new(SharedWriter(writer)));
        handle.write(b"abc").unwrap();
        assert_eq!(handle.seek("current", 0).unwrap(), 3);
        assert!(handle.seek("start", 0).is_err());
        Box::new(handle).commit().unwrap();
        assert_eq!(*output.lock().unwrap(), b"abc");
    }

    struct SharedWriter(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl Write for SharedWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
}
