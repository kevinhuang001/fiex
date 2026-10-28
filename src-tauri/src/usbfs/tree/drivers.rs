use super::*;

pub(super) fn ntfs_list(device: &str, path: &str) -> Result<Vec<UsbEntry>, String> {
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

pub(super) fn ntfs_read(device: &str, path: &str) -> Result<Vec<u8>, String> {
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

pub(super) fn ext4_list(device: &str, path: &str) -> Result<Vec<UsbEntry>, String> {
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

pub(super) fn ext4_read(device: &str, path: &str) -> Result<Vec<u8>, String> {
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

pub(super) fn btrfs_list(device: &str, path: &str) -> Result<Vec<UsbEntry>, String> {
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

pub(super) fn btrfs_read(device: &str, path: &str) -> Result<Vec<u8>, String> {
    let fs = btrfs_fs(device)?;
    let path = clean_path(path);
    let inode = fs.lookup_path(&path).map_err(|error| error.to_string())?;
    fs.read_file(inode.ino).map_err(|error| error.to_string())
}

pub(super) fn apple_list(device: &str, path: &str, kind: FsKind) -> Result<Vec<UsbEntry>, String> {
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
        _ => Err(crate::i18n::t("不是 Apple 文件系统").into()),
    }
}

pub(super) fn apple_read(device: &str, path: &str, kind: FsKind) -> Result<Vec<u8>, String> {
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
        _ => Err(crate::i18n::t("不是 Apple 文件系统").into()),
    }
}
