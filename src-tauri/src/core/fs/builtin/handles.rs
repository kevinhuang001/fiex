//! Bounded storage stream handles.
use super::*;
use std::io::{Read, Seek, SeekFrom, Write};
pub(super) struct AppRead {
    pub(super) file: fs::File,
    pub(super) size: u64,
}
impl ReadHandle for AppRead {
    fn size(&self) -> u64 {
        self.size
    }
    fn read_at(&mut self, offset: u64, length: usize) -> Result<(Vec<u8>, bool), String> {
        self.file
            .seek(SeekFrom::Start(offset))
            .map_err(|e| e.to_string())?;
        let mut data = vec![0; length.min(CHUNK)];
        let n = self.file.read(&mut data).map_err(|e| e.to_string())?;
        data.truncate(n);
        Ok((data, offset.saturating_add(n as u64) >= self.size))
    }
}
pub(super) struct AppWrite {
    pub(super) temporary: Option<tempfile::NamedTempFile>,
    pub(super) target: PathBuf,
    pub(super) create: bool,
}
impl WriteHandle for AppWrite {
    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.temporary
            .as_mut()
            .ok_or("stream closed")?
            .as_file_mut()
            .write_all(bytes)
            .map_err(|e| e.to_string())
    }
    fn seek(&mut self, kind: &str, offset: i64) -> Result<u64, String> {
        let from = match kind {
            "start" => SeekFrom::Start(u64::try_from(offset).map_err(|e| e.to_string())?),
            "current" => SeekFrom::Current(offset),
            "end" => SeekFrom::End(offset),
            _ => return Err("Unsupported: invalid seek operation".into()),
        };
        self.temporary
            .as_mut()
            .ok_or("stream closed")?
            .as_file_mut()
            .seek(from)
            .map_err(|e| e.to_string())
    }
    fn commit(mut self: Box<Self>) -> Result<Value, String> {
        let temp = self.temporary.take().ok_or("stream closed")?;
        temp.as_file().sync_all().map_err(|e| e.to_string())?;
        if self.create {
            temp.persist_noclobber(&self.target)
                .map_err(|e| e.error.to_string())?;
        } else {
            temp.persist(&self.target)
                .map_err(|e| e.error.to_string())?;
        }
        Ok(json!(null))
    }
}
