use super::*;
use fiex_provider_contracts::{FsRead, FsWrite};
use std::{io::Read, sync::Mutex};

const MAX_CHUNK: usize = 4 * 1024 * 1024;

/// An open read stream. Dropping it releases the provider-side handle.
pub struct FsStream {
    reader: Mutex<Box<dyn Read + Send>>,
    offset: Mutex<u64>,
    size: Option<u64>,
}

impl FsStream {
    pub(super) fn new(read: FsRead) -> Self {
        Self {
            reader: Mutex::new(read.reader),
            offset: Mutex::new(0),
            size: read.size,
        }
    }

    pub fn size(&self) -> Option<u64> {
        self.size
    }

    /// Read up to `max_bytes` starting at the absolute `offset`.
    pub fn read_at(&self, offset: u64, max_bytes: usize) -> Result<(Vec<u8>, bool), String> {
        if max_bytes == 0 {
            return Ok((Vec::new(), false));
        }
        let max_bytes = max_bytes.min(MAX_CHUNK);
        let mut cursor = self
            .offset
            .lock()
            .map_err(|_| "read stream unavailable".to_string())?;
        if offset < *cursor {
            return Err("stream cannot seek backwards".into());
        }
        let mut reader = self
            .reader
            .lock()
            .map_err(|_| "read stream unavailable".to_string())?;
        let mut discard = [0u8; 8192];
        while *cursor < offset {
            let remaining = (offset - *cursor).min(discard.len() as u64) as usize;
            let read = reader
                .read(&mut discard[..remaining])
                .map_err(|error| error.to_string())?;
            if read == 0 {
                return Ok((Vec::new(), true));
            }
            *cursor += read as u64;
        }
        let mut bytes = vec![0; max_bytes];
        let read = reader.read(&mut bytes).map_err(|error| error.to_string())?;
        bytes.truncate(read);
        *cursor += read as u64;
        Ok((bytes, read == 0))
    }

    pub fn close(self) {}
}

/// An open write stream. Content becomes visible only after `commit`.
pub struct FsWriteStream {
    write: Option<FsWrite>,
    offset: u64,
    finished: bool,
}

impl FsWriteStream {
    pub(super) fn new(write: FsWrite) -> Self {
        Self {
            write: Some(write),
            offset: 0,
            finished: false,
        }
    }

    pub fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        use std::io::Write;
        let write = self
            .write
            .as_mut()
            .ok_or_else(|| "write stream is already finished".to_string())?;
        let writer = &mut write.writer;
        write_chunks(bytes, &mut self.offset, |_, chunk| {
            writer
                .write(chunk)
                .map(|written| written as u64)
                .map_err(|error| error.to_string())
        })
    }

    pub fn commit(mut self) -> Result<Option<FsEntry>, String> {
        let write = self
            .write
            .take()
            .ok_or_else(|| "write stream is already finished".to_string())?;
        let entry = write.commit().map_err(|error| error.to_string())?;
        self.finished = true;
        entry
            .filter(|entry| !entry.node_id.is_empty())
            .map(|entry| {
                serde_json::to_value(entry)
                    .map_err(|error| error.to_string())
                    .and_then(super::types::parse_entry)
            })
            .transpose()
    }

    pub fn abort(mut self) -> Result<(), String> {
        if let Some(write) = self.write.take() {
            write.abort();
        }
        self.finished = true;
        Ok(())
    }
}

impl Drop for FsWriteStream {
    fn drop(&mut self) {
        if !self.finished {
            if let Some(write) = self.write.take() {
                write.abort();
            }
            self.finished = true;
        }
    }
}

// A successful write consumes all input. Providers may accept partial chunks,
// but cannot report zero progress, oversized writes or overflow the offset.
fn write_chunks(
    mut bytes: &[u8],
    offset: &mut u64,
    mut send: impl FnMut(u64, &[u8]) -> Result<u64, String>,
) -> Result<(), String> {
    while !bytes.is_empty() {
        let chunk = &bytes[..bytes.len().min(MAX_CHUNK)];
        let written = send(*offset, chunk)?;
        if written == 0 || written > chunk.len() as u64 {
            return Err("provider returned an invalid written count".into());
        }
        *offset = offset.checked_add(written).ok_or("write offset overflow")?;
        bytes = &bytes[written as usize..];
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_all_large_input_even_when_provider_accepts_partial_chunks() {
        let data = vec![42; MAX_CHUNK + 71];
        let mut received = Vec::new();
        let mut offset = 0;
        write_chunks(&data, &mut offset, |position, chunk| {
            assert_eq!(position, received.len() as u64);
            assert!(chunk.len() <= MAX_CHUNK);
            let written = chunk.len().min(777_777);
            received.extend_from_slice(&chunk[..written]);
            Ok(written as u64)
        })
        .unwrap();
        assert_eq!(received, data);
        assert_eq!(offset, data.len() as u64);
    }

    #[test]
    fn invalid_write_counts_and_offset_overflow_are_errors() {
        for reported in [0, 4] {
            let mut offset = 0;
            assert!(write_chunks(b"abc", &mut offset, |_, _| Ok(reported)).is_err());
            assert_eq!(offset, 0);
        }
        let mut offset = u64::MAX;
        assert!(write_chunks(b"a", &mut offset, |_, _| Ok(1)).is_err());
        assert_eq!(offset, u64::MAX);
    }
}
