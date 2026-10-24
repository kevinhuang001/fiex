//! Read-only listing, lookup, search and copy-out operations.

use super::mount::{btrfs_fs, ensure_access, ext4_mount};
use super::util::{clean_path, join_rel, FsKind, Route, UsbEntry};
use super::write::remove;
use std::path::{Path, PathBuf};

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

fn btrfs_list(device: &str, path: &str) -> Result<Vec<UsbEntry>, String> {
    let fs = btrfs_fs(device)?;
    let path = clean_path(path);
    let inode = fs.lookup_path(&path).map_err(|error| error.to_string())?;
    let mut out = Vec::new();
    for entry in fs.read_dir(inode.ino).map_err(|error| error.to_string())? {
        let child = fs
            .read_inode(entry.ino)
            .map_err(|error| error.to_string())?;
        out.push(UsbEntry {
            name: String::from_utf8_lossy(&entry.name).into_owned(),
            is_dir: child.mode & 0o170000 == 0o040000,
            size: child.size,
            modified: 0,
            readonly: true,
        });
    }
    Ok(out)
}

fn btrfs_read(device: &str, path: &str) -> Result<Vec<u8>, String> {
    let fs = btrfs_fs(device)?;
    let path = clean_path(path);
    let inode = fs.lookup_path(&path).map_err(|error| error.to_string())?;
    fs.read_file(inode.ino).map_err(|error| error.to_string())
}

fn apple_list(device: &str, path: &str, kind: FsKind) -> Result<Vec<UsbEntry>, String> {
    let file = std::fs::File::open(device).map_err(|error| error.to_string())?;
    let reader = std::io::BufReader::new(file);
    let path = clean_path(path);
    match kind {
        FsKind::Apfs => {
            let mut volume = apfs::ApfsVolume::open(reader).map_err(|error| error.to_string())?;
            Ok(volume
                .list_directory(&path)
                .map_err(|error| error.to_string())?
                .into_iter()
                .map(|entry| UsbEntry {
                    name: entry.name,
                    is_dir: matches!(entry.kind, apfs::EntryKind::Directory),
                    size: entry.size,
                    modified: entry.modify_time.max(0) as u64,
                    readonly: true,
                })
                .collect())
        }
        FsKind::Hfs => {
            let mut volume = hfsplus::HfsVolume::open(reader).map_err(|error| error.to_string())?;
            Ok(volume
                .list_directory(&path)
                .map_err(|error| error.to_string())?
                .into_iter()
                .map(|entry| UsbEntry {
                    name: entry.name,
                    is_dir: matches!(entry.kind, hfsplus::EntryKind::Directory),
                    size: entry.size,
                    modified: (entry.modify_date as i64 - 2_082_844_800).max(0) as u64,
                    readonly: true,
                })
                .collect())
        }
        _ => Err("不是 Apple 文件系统".into()),
    }
}

fn apple_read(device: &str, path: &str, kind: FsKind) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(device).map_err(|error| error.to_string())?;
    let reader = std::io::BufReader::new(file);
    let path = clean_path(path);
    match kind {
        FsKind::Apfs => apfs::ApfsVolume::open(reader)
            .map_err(|error| error.to_string())?
            .read_file(&path)
            .map_err(|error| error.to_string()),
        FsKind::Hfs => hfsplus::HfsVolume::open(reader)
            .map_err(|error| error.to_string())?
            .read_file(&path)
            .map_err(|error| error.to_string()),
        _ => Err("不是 Apple 文件系统".into()),
    }
}

pub fn list(device: &str, kind: FsKind, path: &str) -> Result<Vec<UsbEntry>, String> {
    if !Path::new(device).exists() {
        return Err("设备不存在".into());
    }
    match kind {
        FsKind::Ntfs => ntfs_list(device, path),
        FsKind::Ext4 => ext4_list(device, path),
        FsKind::Apfs => apple_list(device, path, FsKind::Apfs),
        FsKind::Hfs => apple_list(device, path, FsKind::Hfs),
        FsKind::Btrfs => btrfs_list(device, path),
    }
}

pub fn read(device: &str, kind: FsKind, path: &str) -> Result<Vec<u8>, String> {
    match kind {
        FsKind::Ntfs => ntfs_read(device, path),
        FsKind::Ext4 => ext4_read(device, path),
        FsKind::Apfs => apple_read(device, path, FsKind::Apfs),
        FsKind::Hfs => apple_read(device, path, FsKind::Hfs),
        FsKind::Btrfs => btrfs_read(device, path),
    }
}

pub fn read_path(route: &Route) -> Result<Vec<u8>, String> {
    ensure_access(route)?;
    read(&route.driver_device, route.kind, &route.rel)
}

pub fn listing(route: &Route) -> Result<Vec<crate::files::Entry>, String> {
    ensure_access(route)?;
    let files = list(&route.driver_device, route.kind, &route.rel)?;
    Ok(files
        .into_iter()
        .map(|file| {
            let rel = join_rel(&route.rel, &file.name);
            let path = format!("{}/{}", route.root.trim_end_matches('/'), rel);
            crate::files::Entry {
                path,
                extension: Path::new(&file.name)
                    .extension()
                    .map(|value| value.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                mime: mime_guess::from_path(&file.name)
                    .first_or_octet_stream()
                    .to_string(),
                name: file.name,
                is_dir: file.is_dir,
                is_symlink: false,
                size: file.size,
                modified: file.modified,
                created: file.modified,
                readonly: file.readonly,
                link_target: None,
                tags: vec![],
                sync: None,
            }
        })
        .collect())
}

pub fn entry(route: &Route) -> Result<crate::files::Entry, String> {
    let (parent, name) = route
        .rel
        .rsplit_once('/')
        .unwrap_or(("", route.rel.as_str()));
    let parent_route = Route {
        rel: parent.into(),
        ..route.clone()
    };
    let entries = listing(&parent_route)?;
    entries
        .into_iter()
        .find(|entry| entry.name == name)
        .ok_or_else(|| "文件不存在".to_string())
}

pub fn search(
    route: &Route,
    query: &str,
    limit: usize,
) -> Result<Vec<crate::files::Entry>, String> {
    let needle = query.to_lowercase();
    let mut out = Vec::new();
    let mut pending = vec![route.clone()];
    while let Some(directory) = pending.pop() {
        for entry in listing(&directory)? {
            if entry.name.to_lowercase().contains(&needle) {
                out.push(entry.clone());
                if out.len() >= limit {
                    return Ok(out);
                }
            }
            if entry.is_dir {
                pending.push(Route {
                    rel: join_rel(&directory.rel, &entry.name),
                    ..directory.clone()
                });
            }
        }
    }
    Ok(out)
}

fn local_unique(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let stem = path
        .file_stem()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "item".into());
    let ext = path
        .extension()
        .map(|value| format!(".{}", value.to_string_lossy()))
        .unwrap_or_default();
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    for index in 2.. {
        let candidate = parent.join(format!("{stem} ({index}){ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

fn copy_route(source: &Route, target: &Path) -> Result<(), String> {
    let route_parent = source
        .rel
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .unwrap_or("");
    let name = source
        .rel
        .rsplit_once('/')
        .map(|(_, name)| name)
        .unwrap_or(source.rel.as_str());
    let entries = list(&source.driver_device, source.kind, route_parent)?;
    if let Some(entry) = entries.into_iter().find(|entry| entry.name == name) {
        if entry.is_dir {
            std::fs::create_dir_all(target).map_err(|error| error.to_string())?;
            for child in list(&source.driver_device, source.kind, &source.rel)? {
                let child_route = Route {
                    rel: join_rel(&source.rel, &child.name),
                    ..source.clone()
                };
                copy_route(&child_route, &target.join(child.name))?;
            }
        } else {
            let data = read(&source.driver_device, source.kind, &source.rel)?;
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            std::fs::write(target, data).map_err(|error| error.to_string())?;
        }
        Ok(())
    } else {
        Err("文件不存在".into())
    }
}

pub fn copy_to_local(
    source: &Route,
    destination: &Path,
    keep_both: bool,
    moving: bool,
) -> Result<String, String> {
    ensure_access(source)?;
    let name = source
        .rel
        .rsplit_once('/')
        .map(|(_, name)| name)
        .unwrap_or(source.rel.as_str());
    if name.is_empty() {
        return Err("不能复制整个虚拟卷根目录".into());
    }
    let target = if keep_both {
        local_unique(&destination.join(name))
    } else {
        let target = destination.join(name);
        if target.exists() {
            return Err("目标位置已有同名项目".into());
        }
        target
    };
    copy_route(source, &target)?;
    if moving {
        remove(source)?;
    }
    Ok(target.to_string_lossy().into_owned())
}
