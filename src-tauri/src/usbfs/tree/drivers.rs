use super::*;

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
