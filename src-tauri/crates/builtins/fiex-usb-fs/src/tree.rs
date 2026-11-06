//! Filesystem-driver listing and bounded read operations.

use super::mount::{btrfs_fs, ensure_access};
use super::util::{clean_path, FsKind, Route, UsbEntry};
use std::path::Path;

mod drivers;
mod stream;
use drivers::*;
pub use stream::{copy_file_to, read_at};

pub fn list(device: &str, kind: FsKind, path: &str) -> Result<Vec<UsbEntry>, String> {
    let _guard = super::access::lock(device)?;
    if !Path::new(device).exists() {
        return Err("StaleRef: USB device disconnected".into());
    }
    match kind {
        FsKind::Apfs => apple_list(device, path, FsKind::Apfs),
        FsKind::Hfs => apple_list(device, path, FsKind::Hfs),
        FsKind::Btrfs => btrfs_list(device, path),
    }
}

pub fn read(device: &str, kind: FsKind, path: &str) -> Result<Vec<u8>, String> {
    let _guard = super::access::lock(device)?;
    match kind {
        FsKind::Apfs => apple_read(device, path, FsKind::Apfs),
        FsKind::Hfs => apple_read(device, path, FsKind::Hfs),
        FsKind::Btrfs => btrfs_read(device, path),
    }
}
