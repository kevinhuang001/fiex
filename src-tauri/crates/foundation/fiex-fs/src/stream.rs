//! Bounded byte-stream contracts shared by filesystem services.
use serde_json::Value;

/// Maximum number of bytes transferred in one stream chunk.
pub const CHUNK: usize = 256 * 1024;

pub trait ReadHandle: Send {
    fn size(&self) -> u64;
    fn read_at(&mut self, offset: u64, length: usize) -> Result<(Vec<u8>, bool), String>;
}

pub trait WriteHandle: Send {
    fn write(&mut self, bytes: &[u8]) -> Result<(), String>;
    fn seek(&mut self, kind: &str, offset: i64) -> Result<u64, String>;
    fn commit(self: Box<Self>) -> Result<Value, String>;
}
