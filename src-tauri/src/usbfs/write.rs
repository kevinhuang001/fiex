//! Create, rename, delete, write and cache write-back operations.

use super::mount::{ensure_access, ext4_mount};
use super::tree::{entry, read_path};
use super::util::{clean_path, join_rel, FsKind, Route};
use notify::Watcher;
use std::path::{Path, PathBuf};

mod cache;
pub use cache::*;

fn ntfs_write(device: &str, path: &str, data: &[u8]) -> Result<(), String> {
    use fs_ntfs::facade::Filesystem;
    let fs = Filesystem::mount_rw(device).map_err(|error| error.to_string())?;
    let path = clean_path(path);
    if fs.stat(&path).is_err() {
        let (parent, name) = path.rsplit_once('/').ok_or(crate::i18n::t("无效路径"))?;
        fs.create_file(if parent.is_empty() { "/" } else { parent }, name)
            .map_err(|error| error.to_string())?;
    }
    fs.write_file_contents(&path, data)
        .map(|_| ())
        .map_err(|error| error.to_string())
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

pub fn write(device: &str, kind: FsKind, path: &str, data: &[u8]) -> Result<(), String> {
    let _guard = super::access::lock(device)?;
    match kind {
        FsKind::Ntfs => ntfs_write(device, path, data),
        FsKind::Ext4 => ext4_write(device, path, data),
        FsKind::Apfs | FsKind::Hfs | FsKind::Btrfs => {
            Err(crate::i18n::t("该文件系统内置驱动为只读").into())
        }
    }
}

pub fn write_path(route: &Route, data: &[u8]) -> Result<(), String> {
    ensure_access(route)?;
    write(&route.driver_device, route.kind, &route.rel, data)
}

/// Commit a host-side staged stream through a sibling on the device. The
/// device write is bounded, and a failed copy never overwrites the old entry.
pub fn commit_staged(
    route: &Route,
    staged: &mut std::fs::File,
    replace: bool,
) -> Result<(), String> {
    use std::io::{Read, Seek};
    let _guard = super::access::lock(&route.driver_device)?;
    ensure_access(route)?;
    if matches!(route.kind, FsKind::Apfs | FsKind::Hfs | FsKind::Btrfs) {
        return Err("Unsupported: read-only USB filesystem".into());
    }
    let (parent_rel, name) = route.rel.rsplit_once('/').unwrap_or(("", &route.rel));
    if name.is_empty() {
        return Err("cannot overwrite USB root".into());
    }
    let parent = Route {
        rel: parent_rel.into(),
        ..route.clone()
    };
    let token = crate::plugins::mounts::new_id()?;
    let staging_name = format!(".fiex-{token}.tmp");
    let temp = Route {
        rel: join_rel(parent_rel, &staging_name),
        ..route.clone()
    };
    create(&parent, &staging_name, false)?;
    let copy = (|| {
        staged.rewind().map_err(|e| e.to_string())?;
        let mut offset = 0u64;
        let mut buffer = vec![0; crate::core::fs::CHUNK];
        loop {
            let count = staged.read(&mut buffer).map_err(|e| e.to_string())?;
            if count == 0 {
                break;
            }
            let path = clean_path(&format!("/{}", temp.rel));
            let wrote = match temp.kind {
                FsKind::Ntfs => fs_ntfs::write::write_at(
                    Path::new(&temp.driver_device),
                    &path,
                    offset,
                    &buffer[..count],
                )
                .map_err(|e| e.to_string())?,
                FsKind::Ext4 => ext4_mount(&temp.driver_device, true)?
                    .apply_pwrite(&path, offset, &buffer[..count])
                    .map_err(|e| e.to_string())?,
                _ => unreachable!(),
            };
            if wrote != count as u64 {
                return Err("short USB write".into());
            }
            offset += wrote;
        }
        let path = clean_path(&format!("/{}", temp.rel));
        match temp.kind {
            FsKind::Ntfs => {
                fs_ntfs::facade::Filesystem::mount_rw(&temp.driver_device)
                    .map_err(|e| e.to_string())?
                    .truncate(&path, offset)
                    .map_err(|e| e.to_string())?;
            }
            FsKind::Ext4 => {
                let fs = ext4_mount(&temp.driver_device, true)?;
                let ino = fs
                    .lookup_path_bytes(path.as_bytes())
                    .map_err(|e| e.to_string())?;
                fs.apply_truncate_ino(ino, offset)
                    .map_err(|e| e.to_string())?;
            }
            _ => unreachable!(),
        }
        ensure_access(route)?;
        let existed = entry(route).is_ok();
        if existed && !replace {
            return Err("Conflict: destination appeared during write".into());
        }
        let backup_name = format!(".fiex-{token}.bak");
        let backup = Route {
            rel: join_rel(parent_rel, &backup_name),
            ..route.clone()
        };
        if existed {
            rename(route, &backup_name)?;
        }
        if let Err(error) = rename(&temp, name) {
            if existed {
                let _ = rename(&backup, name);
            }
            return Err(error);
        }
        if existed {
            remove(&backup)?;
        }
        Ok(())
    })();
    if copy.is_err() {
        let _ = remove(&temp);
    }
    copy
}

pub fn create(route: &Route, name: &str, folder: bool) -> Result<(), String> {
    let _guard = super::access::lock(&route.driver_device)?;
    ensure_access(route)?;
    if name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        return Err(crate::i18n::t("无效文件名").into());
    }
    let rel = join_rel(&route.rel, name);
    match route.kind {
        FsKind::Ntfs => {
            use fs_ntfs::facade::Filesystem;
            let fs = Filesystem::mount_rw(&route.driver_device).map_err(|e| e.to_string())?;
            let parent = clean_path(&format!("/{}", route.rel));
            if folder {
                fs.mkdir(&parent, name)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            } else {
                fs.create_file(&parent, name)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
        }
        FsKind::Ext4 => {
            let fs = ext4_mount(&route.driver_device, true)?;
            let path = clean_path(&format!("/{rel}"));
            if folder {
                fs.apply_mkdir(&path, 0o40755)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            } else {
                fs.apply_create(&path, 0o100644)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
        }
        FsKind::Apfs | FsKind::Hfs | FsKind::Btrfs => {
            Err(crate::i18n::t("该文件系统内置驱动为只读").into())
        }
    }
}

pub fn remove(route: &Route) -> Result<(), String> {
    let _guard = super::access::lock(&route.driver_device)?;
    ensure_access(route)?;
    let path = clean_path(&format!("/{}", route.rel));
    match route.kind {
        FsKind::Ntfs => {
            use fs_ntfs::facade::Filesystem;
            let fs = Filesystem::mount_rw(&route.driver_device).map_err(|e| e.to_string())?;
            fs.remove(&path).map_err(|e| e.to_string())
        }
        FsKind::Ext4 => {
            let fs = ext4_mount(&route.driver_device, true)?;
            if fs
                .stat_ino(
                    fs.lookup_path_bytes(path.as_bytes())
                        .map_err(|e| e.to_string())?,
                )
                .map(|attrs| attrs.mode & 0o170000 == 0o040000)
                .unwrap_or(false)
            {
                fs.apply_rmdir(&path).map_err(|e| e.to_string())
            } else {
                fs.apply_unlink(&path).map_err(|e| e.to_string())
            }
        }
        FsKind::Apfs | FsKind::Hfs | FsKind::Btrfs => {
            Err(crate::i18n::t("该文件系统内置驱动为只读").into())
        }
    }
}

pub fn rename(route: &Route, new_name: &str) -> Result<(), String> {
    let _guard = super::access::lock(&route.driver_device)?;
    ensure_access(route)?;
    if new_name.contains('/') || new_name.contains('\\') || new_name == "." || new_name == ".." {
        return Err(crate::i18n::t("无效文件名").into());
    }
    let path = clean_path(&format!("/{}", route.rel));
    match route.kind {
        FsKind::Ntfs => {
            use fs_ntfs::facade::Filesystem;
            let fs = Filesystem::mount_rw(&route.driver_device).map_err(|e| e.to_string())?;
            fs.rename(&path, new_name).map_err(|e| e.to_string())
        }
        FsKind::Ext4 => {
            let fs = ext4_mount(&route.driver_device, true)?;
            let parent = path.rsplit_once('/').map(|(p, _)| p).unwrap_or("/");
            let target = clean_path(&format!("{parent}/{new_name}"));
            fs.apply_rename(&path, &target, false)
                .map_err(|e| e.to_string())
        }
        FsKind::Apfs | FsKind::Hfs | FsKind::Btrfs => {
            Err(crate::i18n::t("该文件系统内置驱动为只读").into())
        }
    }
}
