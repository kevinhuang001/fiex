//! In-process USB filesystem support for NTFS and ext4.
//!
//! The drivers operate on a block device or disk image directly; no OS mount
//! and no FUSE are involved. APFS, HFS+ and btrfs remain read-only elsewhere.

use serde::Serialize;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FsKind {
    Ntfs,
    Ext4,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsbEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: u64,
    pub readonly: bool,
}

pub fn detect(filesystem: &str) -> Option<FsKind> {
    match filesystem.to_ascii_lowercase().as_str() {
        "ntfs" | "ntfs3" => Some(FsKind::Ntfs),
        "ext2" | "ext3" | "ext4" => Some(FsKind::Ext4),
        _ => None,
    }
}

fn clean_path(path: &str) -> String {
    let mut out = String::from("/");
    for part in path
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
    {
        if part == ".." {
            out = out
                .rsplit_once('/')
                .map(|(parent, _)| {
                    if parent.is_empty() {
                        "/".into()
                    } else {
                        parent.into()
                    }
                })
                .unwrap_or_else(|| "/".into());
        } else {
            if !out.ends_with('/') {
                out.push('/');
            }
            out.push_str(part);
        }
    }
    out
}

fn ntfs_list(device: &str, path: &str) -> Result<Vec<UsbEntry>, String> {
    use fs_ntfs::facade::Filesystem;
    let fs = Filesystem::mount(device).map_err(|error| error.to_string())?;
    let path = clean_path(path);
    let mut out = Vec::new();
    for item in fs.read_dir(&path).map_err(|error| error.to_string())? {
        if item.name == "." || item.name == ".." {
            continue;
        }
        let full = if path == "/" {
            format!("/{}", item.name)
        } else {
            format!("{path}/{}", item.name)
        };
        let attrs = fs.stat(&full).map_err(|error| error.to_string())?;
        let is_dir = matches!(attrs.file_type, fs_ntfs::facade::FileType::Directory);
        out.push(UsbEntry {
            name: item.name,
            is_dir,
            size: if is_dir { 0 } else { attrs.size },
            modified: attrs.mtime_sec.max(0) as u64,
            readonly: false,
        });
    }
    Ok(out)
}

fn ntfs_read(device: &str, path: &str) -> Result<Vec<u8>, String> {
    use fs_ntfs::facade::Filesystem;
    let fs = Filesystem::mount(device).map_err(|error| error.to_string())?;
    let path = clean_path(path);
    let attrs = fs.stat(&path).map_err(|error| error.to_string())?;
    let mut data = vec![0u8; attrs.size as usize];
    let mut offset = 0u64;
    while (offset as usize) < data.len() {
        let read = fs
            .read_file(&path, offset, &mut data[offset as usize..])
            .map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        offset += read as u64;
    }
    data.truncate(offset as usize);
    Ok(data)
}

fn ntfs_write(device: &str, path: &str, data: &[u8]) -> Result<(), String> {
    use fs_ntfs::facade::Filesystem;
    let fs = Filesystem::mount_rw(device).map_err(|error| error.to_string())?;
    let path = clean_path(path);
    if fs.stat(&path).is_err() {
        let (parent, name) = path.rsplit_once('/').ok_or("无效路径")?;
        fs.create_file(if parent.is_empty() { "/" } else { parent }, name)
            .map_err(|error| error.to_string())?;
    }
    fs.write_file_contents(&path, data)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn ext4_mount(device: &str, writable: bool) -> Result<fs_ext4::Filesystem, String> {
    use fs_ext4::block_io::FileDevice;
    let device = if writable {
        FileDevice::open_rw(device)
    } else {
        FileDevice::open(device)
    }
    .map_err(|error| error.to_string())?;
    fs_ext4::Filesystem::mount(std::sync::Arc::new(device)).map_err(|error| error.to_string())
}

fn ext4_list(device: &str, path: &str) -> Result<Vec<UsbEntry>, String> {
    let fs = ext4_mount(device, false)?;
    let path = clean_path(path);
    let inode = fs
        .lookup_path_bytes(path.as_bytes())
        .map_err(|error| error.to_string())?;
    let mut out = Vec::new();
    for item in fs.read_dir_ino(inode).map_err(|error| error.to_string())? {
        if item.name == b"." || item.name == b".." {
            continue;
        }
        let attrs = fs.stat_ino(item.inode).map_err(|error| error.to_string())?;
        out.push(UsbEntry {
            name: String::from_utf8_lossy(&item.name).into_owned(),
            is_dir: attrs.mode & 0o170000 == 0o040000,
            size: attrs.size,
            modified: attrs.mtime.max(0) as u64,
            readonly: false,
        });
    }
    Ok(out)
}

fn ext4_read(device: &str, path: &str) -> Result<Vec<u8>, String> {
    let fs = ext4_mount(device, false)?;
    let path = clean_path(path);
    let inode = fs
        .lookup_path_bytes(path.as_bytes())
        .map_err(|error| error.to_string())?;
    let attrs = fs.stat_ino(inode).map_err(|error| error.to_string())?;
    let mut data = vec![0u8; attrs.size as usize];
    let mut offset = 0usize;
    while offset < data.len() {
        let read = fs
            .read_ino(inode, offset as u64, &mut data[offset..])
            .map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        offset += read;
    }
    data.truncate(offset);
    Ok(data)
}

fn ext4_write(device: &str, path: &str, data: &[u8]) -> Result<(), String> {
    let fs = ext4_mount(device, true)?;
    let path = clean_path(path);
    if fs.lookup_path_bytes(path.as_bytes()).is_err() {
        fs.apply_create(&path, 0o100644)
            .map_err(|error| error.to_string())?;
    }
    fs.apply_pwrite(&path, 0, data)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub fn list(device: &str, kind: FsKind, path: &str) -> Result<Vec<UsbEntry>, String> {
    if !Path::new(device).exists() {
        return Err("设备不存在".into());
    }
    match kind {
        FsKind::Ntfs => ntfs_list(device, path),
        FsKind::Ext4 => ext4_list(device, path),
    }
}

pub fn read(device: &str, kind: FsKind, path: &str) -> Result<Vec<u8>, String> {
    match kind {
        FsKind::Ntfs => ntfs_read(device, path),
        FsKind::Ext4 => ext4_read(device, path),
    }
}

pub fn write(device: &str, kind: FsKind, path: &str, data: &[u8]) -> Result<(), String> {
    match kind {
        FsKind::Ntfs => ntfs_write(device, path, data),
        FsKind::Ext4 => ext4_write(device, path, data),
    }
}

#[cfg(test)]
mod tests {
    use super::detect;

    #[test]
    fn detects_supported_filesystems() {
        assert!(detect("NTFS").is_some());
        assert!(detect("ext4").is_some());
        assert!(detect("apfs").is_none());
    }
}
