use super::Mount;
use serde_json::json;
use std::sync::Arc;

pub(super) struct RemoteReader {
    mount: Mount,
    stream_id: String,
    offset: u64,
    eof: bool,
    closed: bool,
}

impl RemoteReader {
    pub(super) fn new(mount: Mount, stream_id: String) -> Self {
        Self {
            mount,
            stream_id,
            offset: 0,
            eof: false,
            closed: false,
        }
    }
}

impl std::io::Read for RemoteReader {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        use base64::{engine::general_purpose::STANDARD, Engine};
        if buffer.is_empty() || self.eof {
            return Ok(0);
        }
        let limit = buffer.len().min(4 * 1024 * 1024);
        let reply = self
            .mount
            .invoke_remote(
                "vfs.readStream",
                json!({"streamId":self.stream_id,"offset":self.offset,"maxBytes":limit}),
            )
            .map_err(std::io::Error::other)?;
        let encoded = reply["base64"]
            .as_str()
            .ok_or_else(|| std::io::Error::other("provider stream reply is missing data"))?;
        if encoded.len() > limit.div_ceil(3) * 4 {
            return Err(std::io::Error::other(
                "provider exceeded the requested chunk size",
            ));
        }
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|_| std::io::Error::other("provider returned invalid base64 data"))?;
        if bytes.len() > limit {
            return Err(std::io::Error::other(
                "provider exceeded the requested chunk size",
            ));
        }
        let count = bytes.len();
        buffer[..count].copy_from_slice(&bytes);
        self.offset = self.offset.saturating_add(count as u64);
        self.eof = reply["eof"].as_bool().unwrap_or(false) || count == 0;
        Ok(count)
    }
}

impl Drop for RemoteReader {
    fn drop(&mut self) {
        if !self.closed {
            let _ = self
                .mount
                .invoke_remote("vfs.closeRead", json!({"streamId":self.stream_id}));
            self.closed = true;
        }
    }
}

pub(super) struct RemoteWriter {
    mount: Mount,
    stream_id: String,
    offset: u64,
    finished: Arc<std::sync::atomic::AtomicBool>,
}

impl RemoteWriter {
    pub(super) fn new(
        mount: Mount,
        stream_id: String,
        finished: Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        Self {
            mount,
            stream_id,
            offset: 0,
            finished,
        }
    }
}

impl std::io::Write for RemoteWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        use base64::{engine::general_purpose::STANDARD, Engine};
        if bytes.is_empty() {
            return Ok(0);
        }
        let chunk = &bytes[..bytes.len().min(4 * 1024 * 1024)];
        let reply = self
            .mount
            .invoke_remote(
                "vfs.writeStream",
                json!({"streamId":self.stream_id,"offset":self.offset,"base64":STANDARD.encode(chunk)}),
            )
            .map_err(std::io::Error::other)?;
        let written = reply["written"].as_u64().ok_or_else(|| {
            std::io::Error::other("provider write reply is missing written count")
        })?;
        if written == 0 || written > chunk.len() as u64 {
            return Err(std::io::Error::other(
                "provider returned an invalid written count",
            ));
        }
        self.offset = self
            .offset
            .checked_add(written)
            .ok_or_else(|| std::io::Error::other("write offset overflow"))?;
        Ok(written as usize)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Drop for RemoteWriter {
    fn drop(&mut self) {
        if !self.finished.load(std::sync::atomic::Ordering::Acquire) {
            let _ = self
                .mount
                .invoke_remote("vfs.abort", json!({"streamId":self.stream_id}));
            self.finished
                .store(true, std::sync::atomic::Ordering::Release);
        }
    }
}
