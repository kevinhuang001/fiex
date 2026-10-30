//! Bounded storage stream handles.
use super::*;
pub(super) struct PluginRead {
    pub(super) stream: Option<FsStream>,
    pub(super) mount: Mount,
    pub(super) node: String,
    pub(super) size: u64,
    pub(super) offset: u64,
}
impl ReadHandle for PluginRead {
    fn size(&self) -> u64 {
        self.size
    }
    fn read_at(&mut self, offset: u64, length: usize) -> Result<(Vec<u8>, bool), String> {
        if offset >= self.size {
            return Ok((vec![], true));
        }
        // A sequential provider can seek by reopening/replaying bounded chunks.
        if offset < self.offset {
            self.stream.take();
            self.stream = Some(self.mount.open_read(&self.node)?);
            self.offset = 0;
        }
        let stream = self.stream.as_ref().ok_or("read handle closed")?;
        while self.offset < offset {
            let want = (offset - self.offset).min(CHUNK as u64) as usize;
            let (bytes, eof) = stream.read_at(self.offset, want)?;
            self.offset += bytes.len() as u64;
            if eof || bytes.is_empty() {
                return Err("Conflict: stream ended before requested offset".into());
            }
        }
        let result = stream.read_at(offset, length.min(CHUNK))?;
        self.offset = self
            .offset
            .checked_add(result.0.len() as u64)
            .ok_or("read offset overflow")?;
        Ok(result)
    }
}
