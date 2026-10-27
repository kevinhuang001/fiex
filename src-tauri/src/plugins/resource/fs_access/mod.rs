//! Host-owned, directory-confined file access. Directory handles prevent
//! symlink replacement from escaping the captured task root.
use base64::{engine::general_purpose::STANDARD, Engine};
use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
    sync::{Mutex, OnceLock},
};
const MAX_CHUNK: usize = 256 * 1024;
const MAX_HANDLES: usize = 32;

mod operations;
mod reads;
mod scope;
mod writes;
use operations::{directory, entry, relative};
pub use operations::{is_directory, list, mkdir, read_chunk, remove, rename, stat, stat_batch};
pub use reads::{close_read, http_body, open_read, read_open};
pub use scope::{canonical_selection, Scope};
pub use writes::{
    abort, begin_write, commit, seek_write, write_buffer, write_chunk, writer_permission,
};
pub fn cleanup(owner: &str) {
    if let Ok(mut pending) = reads::reads().lock() {
        pending.retain(|_, v| v.owner != owner);
    }
    if let Ok(mut pending) = writes::writes().lock() {
        pending.retain(|_, v| v.owner != owner);
    }
}

#[cfg(test)]
mod tests;
