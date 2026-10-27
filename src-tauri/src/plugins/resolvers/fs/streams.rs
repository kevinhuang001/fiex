use super::*;

const MAX_CHUNK: usize = 4 * 1024 * 1024;

/// An open read stream. Dropping it releases the provider-side handle.
pub struct FsStream {
    pub(super) mount: Mount,
    pub(super) stream_id: String,
    pub(super) size: Option<u64>,
    pub(super) closed: bool,
}

impl FsStream {
    pub fn size(&self) -> Option<u64> {
        self.size
    }

    /// Read up to `max_bytes` starting at the absolute `offset`.
    pub fn read_at(&self, offset: u64, max_bytes: usize) -> Result<(Vec<u8>, bool), String> {
        use base64::{engine::general_purpose::STANDARD, Engine};
        if max_bytes == 0 {
            return Ok((Vec::new(), false));
        }
        let max_bytes = max_bytes.min(MAX_CHUNK);
        let value = self.mount.invoke(
            "fs.readStream",
            json!({
                "streamId": self.stream_id,
                "offset": offset,
                "maxBytes": max_bytes,
            }),
        )?;
        let encoded = value
            .get("base64")
            .and_then(Value::as_str)
            .ok_or("provider stream reply is missing data")?;
        if encoded.len() > max_bytes.div_ceil(3) * 4 {
            return Err("provider exceeded the requested chunk size".into());
        }
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|_| "provider returned invalid base64 data".to_string())?;
        if bytes.len() > max_bytes {
            return Err("provider exceeded the requested chunk size".into());
        }
        let eof = value.get("eof").and_then(Value::as_bool).unwrap_or(false) || bytes.is_empty();
        Ok((bytes, eof))
    }

    pub fn close(mut self) {
        self.release();
    }

    fn release(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        let _ = self
            .mount
            .invoke("fs.closeRead", json!({"streamId": self.stream_id}));
    }
}

impl Drop for FsStream {
    fn drop(&mut self) {
        self.release();
    }
}

/// An open write stream. Content becomes visible only after `commit`.
pub struct FsWriteStream {
    pub(super) mount: Mount,
    pub(super) stream_id: String,
    pub(super) offset: u64,
    pub(super) finished: bool,
}

impl FsWriteStream {
    pub fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        use base64::{engine::general_purpose::STANDARD, Engine};
        write_chunks(bytes, &mut self.offset, |offset, chunk| {
            let value = self.mount.invoke(
                "fs.writeStream",
                json!({
                    "streamId": self.stream_id,
                    "offset": offset,
                    "base64": STANDARD.encode(chunk),
                }),
            )?;
            value
                .get("written")
                .and_then(Value::as_u64)
                .ok_or_else(|| "provider write reply is missing written count".into())
        })
    }

    pub fn commit(mut self) -> Result<Option<FsEntry>, String> {
        let value = self
            .mount
            .invoke("fs.commit", json!({"streamId": self.stream_id}))?;
        self.finished = true;
        if value.is_null() || value.as_object().is_some_and(|object| object.is_empty()) {
            return Ok(None);
        }
        FsEntry::parse(value).map(Some)
    }

    pub fn abort(mut self) -> Result<(), String> {
        self.mount
            .invoke("fs.abort", json!({"streamId": self.stream_id}))?;
        self.finished = true;
        Ok(())
    }
}

impl Drop for FsWriteStream {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self
                .mount
                .invoke("fs.abort", json!({"streamId": self.stream_id}));
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
