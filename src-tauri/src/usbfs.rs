//! In-process USB filesystem support for NTFS and ext4.
//!
//! The drivers operate on a block device or disk image directly; no OS mount
//! and no FUSE are involved. APFS, HFS+ and btrfs remain read-only elsewhere.

use notify::Watcher;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FsKind {
    Ntfs,
    Ext4,
    Apfs,
    Hfs,
    Btrfs,
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

pub fn probe(device: &str) -> Option<FsKind> {
    use std::io::Read;
    let mut file = std::fs::File::open(device).ok()?;
    let mut head = vec![0u8; 70_000];
    let read = file.read(&mut head).ok()?;
    head.truncate(read);
    if head.get(3..11) == Some(b"NTFS    ") {
        return Some(FsKind::Ntfs);
    }
    if head.get(1080..1082) == Some(&[0x53, 0xef]) {
        return Some(FsKind::Ext4);
    }
    if head.get(65_600..65_608) == Some(b"_BHRfS_M") {
        return Some(FsKind::Btrfs);
    }
    if head.get(32..36) == Some(b"NXSB") {
        return Some(FsKind::Apfs);
    }
    if head.get(1024..1026) == Some(b"H+") {
        return Some(FsKind::Hfs);
    }
    None
}

pub fn detect(filesystem: &str) -> Option<FsKind> {
    match filesystem.to_ascii_lowercase().as_str() {
        "ntfs" | "ntfs3" => Some(FsKind::Ntfs),
        "ext2" | "ext3" | "ext4" | "linux" | "ext" => Some(FsKind::Ext4),
        "apfs" => Some(FsKind::Apfs),
        "btrfs" => Some(FsKind::Btrfs),
        "hfs" | "hfs+" | "hfsplus" | "hfsx" | "mac os extended" | "journaled hfs+" => {
            Some(FsKind::Hfs)
        }
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

fn btrfs_fs(device: &str) -> Result<fs_btrfs::Filesystem, String> {
    use fs_ext4::block_io::FileDevice;
    let device = FileDevice::open(device).map_err(|error| error.to_string())?;
    fs_btrfs::Filesystem::mount(std::sync::Arc::new(device)).map_err(|error| error.to_string())
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

pub fn write(device: &str, kind: FsKind, path: &str, data: &[u8]) -> Result<(), String> {
    match kind {
        FsKind::Ntfs => ntfs_write(device, path, data),
        FsKind::Ext4 => ext4_write(device, path, data),
        FsKind::Apfs | FsKind::Hfs | FsKind::Btrfs => Err("该文件系统内置驱动为只读".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_supported_filesystems() {
        assert!(detect("NTFS").is_some());
        assert!(detect("ext4").is_some());
        assert!(detect("apfs").is_some());
        assert!(detect("hfs+").is_some());
        assert!(detect("btrfs").is_some());
    }

    #[test]
    fn btrfs_image_lists_readonly() {
        use fs_ext4::block_io::FileDevice;
        use std::fs::File;
        let temp = tempfile::tempdir().unwrap();
        let image = temp.path().join("usb-btrfs.img");
        let file = File::create(&image).unwrap();
        file.set_len(256 * 1024 * 1024).unwrap();
        drop(file);
        let device = image.to_string_lossy().into_owned();
        {
            let dev = FileDevice::open_rw(&device).unwrap();
            fs_btrfs::mkfs::format(&dev, &fs_btrfs::mkfs::Options::default()).unwrap();
        }
        assert_eq!(probe(&device), Some(FsKind::Btrfs));
        let route = Route {
            device: device.clone(),
            driver_device: device.clone(),
            kind: FsKind::Btrfs,
            rel: String::new(),
            root: virtual_root(&device, FsKind::Btrfs),
        };
        assert!(listing(&route).is_ok());
    }

    #[test]
    fn virtual_route_end_to_end() {
        use fs_ntfs::block_io::{BlockIo, PathIo};
        use std::fs::File;
        let temp = tempfile::tempdir().unwrap();
        let image = temp.path().join("virtual-usb.img");
        let file = File::create(&image).unwrap();
        file.set_len(64 * 1024 * 1024).unwrap();
        drop(file);
        let mut io = PathIo::open_rw(&image).unwrap();
        fs_ntfs::mkfs::format_filesystem(
            &mut io,
            64 * 1024 * 1024,
            4096,
            4096,
            Some("USB"),
            Some(1),
        )
        .unwrap();
        io.sync().unwrap();
        drop(io);
        let device = image.to_string_lossy().into_owned();
        assert_eq!(probe(&device), Some(FsKind::Ntfs));
        let root = virtual_root(&device, FsKind::Ntfs);
        let root_route = route(&root).expect("virtual root route");
        assert_eq!(root_route.device, device);
        create(&root_route, "notes.txt", false).unwrap();
        let file_path = format!("{root}/notes.txt");
        let file_route = route(&file_path).expect("file route");
        write_path(&file_route, b"virtual usb").unwrap();
        assert_eq!(read_path(&file_route).unwrap(), b"virtual usb");
        let entries = listing(&root_route).unwrap();
        assert!(entries.iter().any(|entry| entry.path == file_path));
        assert!(search(&root_route, "notes", 10)
            .unwrap()
            .iter()
            .any(|entry| entry.name == "notes.txt"));
        let local = temp.path().join("local");
        std::fs::create_dir(&local).unwrap();
        copy_to_local(&file_route, &local, false, false).unwrap();
        assert_eq!(
            std::fs::read(local.join("notes.txt")).unwrap(),
            b"virtual usb"
        );
    }

    #[test]
    fn ext4_image_roundtrip() {
        use fs_ext4::block_io::FileDevice;
        use std::fs::File;
        let temp = tempfile::tempdir().unwrap();
        let image = temp.path().join("usb-ext4.img");
        let file = File::create(&image).unwrap();
        file.set_len(64 * 1024 * 1024).unwrap();
        drop(file);
        let device = image.to_string_lossy().into_owned();
        let dev = FileDevice::open_rw(&device).unwrap();
        fs_ext4::mkfs::format_filesystem(&dev, Some("FIEX"), None, 64 * 1024 * 1024, 4096).unwrap();
        drop(dev);
        let root = Route {
            device: device.clone(),
            driver_device: device.clone(),
            kind: FsKind::Ext4,
            rel: String::new(),
            root: virtual_root(&device, FsKind::Ext4),
        };
        create(&root, "hello.txt", false).unwrap();
        let file = Route {
            rel: "hello.txt".into(),
            ..root.clone()
        };
        write_path(&file, b"hello ext4").unwrap();
        assert_eq!(read_path(&file).unwrap(), b"hello ext4");
        assert!(listing(&root)
            .unwrap()
            .iter()
            .any(|entry| entry.name == "hello.txt"));
        rename(&file, "renamed.txt").unwrap();
        let renamed = Route {
            rel: "renamed.txt".into(),
            ..root.clone()
        };
        assert_eq!(read_path(&renamed).unwrap(), b"hello ext4");
        remove(&renamed).unwrap();
    }

    #[test]
    fn ntfs_image_roundtrip() {
        use fs_ntfs::block_io::{BlockIo, PathIo};
        use std::fs::File;
        let temp = tempfile::tempdir().unwrap();
        let image = temp.path().join("usb.img");
        let file = File::create(&image).unwrap();
        file.set_len(64 * 1024 * 1024).unwrap();
        drop(file);
        let mut io = PathIo::open_rw(&image).unwrap();
        fs_ntfs::mkfs::format_filesystem(
            &mut io,
            64 * 1024 * 1024,
            4096,
            4096,
            Some("FIEX"),
            Some(1),
        )
        .unwrap();
        io.sync().unwrap();
        drop(io);
        let device = image.to_string_lossy().into_owned();
        let root = Route {
            device: device.clone(),
            driver_device: device.clone(),
            kind: FsKind::Ntfs,
            rel: String::new(),
            root: virtual_root(&device, FsKind::Ntfs),
        };
        create(&root, "hello.txt", false).unwrap();
        let file = Route {
            rel: "hello.txt".into(),
            ..root.clone()
        };
        write_path(&file, b"hello usb").unwrap();
        assert_eq!(read_path(&file).unwrap(), b"hello usb");
        assert!(listing(&root)
            .unwrap()
            .iter()
            .any(|entry| entry.name == "hello.txt"));
        rename(&file, "renamed.txt").unwrap();
        let renamed = Route {
            rel: "renamed.txt".into(),
            ..root.clone()
        };
        assert_eq!(read_path(&renamed).unwrap(), b"hello usb");
        remove(&renamed).unwrap();
    }
}

#[derive(Clone, Debug)]
pub struct Route {
    pub device: String,
    pub driver_device: String,
    pub kind: FsKind,
    pub rel: String,
    pub root: String,
}

fn hex_encode(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn hex_decode(value: &str) -> Option<String> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    let bytes = (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).ok())
        .collect::<Option<Vec<_>>>()?;
    String::from_utf8(bytes).ok()
}

pub fn virtual_root(device: &str, kind: FsKind) -> String {
    let base = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".fiex")
        .join("usbfs");
    let kind = match kind {
        FsKind::Ntfs => "ntfs",
        FsKind::Ext4 => "ext4",
        FsKind::Apfs => "apfs",
        FsKind::Hfs => "hfs",
        FsKind::Btrfs => "btrfs",
    };
    base.join(format!("{}-{kind}", hex_encode(device)))
        .to_string_lossy()
        .into_owned()
}

pub fn route(path: &str) -> Option<Route> {
    let base = dirs::home_dir()?
        .join(".fiex")
        .join("usbfs")
        .to_string_lossy()
        .into_owned();
    let relative = path.strip_prefix(&base)?.trim_start_matches(['/', '\\']);
    let (id, rel) = relative
        .split_once('/')
        .or_else(|| relative.split_once('\\'))
        .unwrap_or((relative, ""));
    let rel = rel.replace('\\', "/");
    let (encoded, kind) = id.rsplit_once('-')?;
    let device = hex_decode(encoded)?;
    let kind = match kind {
        "ntfs" => FsKind::Ntfs,
        "ext4" => FsKind::Ext4,
        "apfs" => FsKind::Apfs,
        "hfs" => FsKind::Hfs,
        "btrfs" => FsKind::Btrfs,
        _ => return None,
    };
    let driver_device = if cfg!(target_os = "macos") {
        device.replacen("/dev/disk", "/dev/rdisk", 1)
    } else {
        device.clone()
    };
    Some(Route {
        device,
        driver_device,
        kind,
        rel,
        root: Path::new(&base).join(id).to_string_lossy().into_owned(),
    })
}

fn authorized() -> &'static std::sync::Mutex<std::collections::HashSet<String>> {
    static SET: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> =
        std::sync::OnceLock::new();
    SET.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
}

#[allow(dead_code)]
fn can_open(device: &str) -> bool {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(device)
        .is_ok()
}

#[cfg(target_os = "macos")]
fn run_privileged(script: &str) -> Result<(), String> {
    let output = std::process::Command::new("osascript")
        .args(["-e", script])
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().into())
    }
}

/// Unmount a volume and grant this process raw read/write access through the
/// native administrator / polkit prompt. Runs once per device per session.
pub fn ensure_access(route: &Route) -> Result<(), String> {
    if authorized().lock().unwrap().contains(&route.device) {
        return Ok(());
    }
    if can_open(&route.driver_device) {
        authorized().lock().unwrap().insert(route.device.clone());
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("diskutil")
            .args(["unmount", &route.device])
            .output();
        let script = format!(
            "do shell script \"chmod 666 {}\" with administrator privileges",
            route.driver_device
        );
        run_privileged(&script)?;
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("udisksctl")
            .args(["unmount", "-b", &route.device])
            .output();
        let output = std::process::Command::new("pkexec")
            .args(["chmod", "a+rw", &route.driver_device])
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().into());
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = route;
        return Err("当前系统需要通过管理员权限访问原始设备".into());
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        if !can_open(&route.driver_device) {
            return Err("无法获得设备读写权限".into());
        }
        authorized().lock().unwrap().insert(route.device.clone());
        Ok(())
    }
}

fn join_rel(rel: &str, name: &str) -> String {
    if rel.is_empty() {
        name.into()
    } else {
        format!("{}/{}", rel.trim_matches('/'), name)
    }
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

pub fn read_path(route: &Route) -> Result<Vec<u8>, String> {
    ensure_access(route)?;
    read(&route.driver_device, route.kind, &route.rel)
}

pub fn write_path(route: &Route, data: &[u8]) -> Result<(), String> {
    ensure_access(route)?;
    write(&route.driver_device, route.kind, &route.rel, data)
}

pub fn create(route: &Route, name: &str, folder: bool) -> Result<(), String> {
    ensure_access(route)?;
    if name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        return Err("无效文件名".into());
    }
    let rel = join_rel(&route.rel, name);
    match route.kind {
        FsKind::Ntfs => {
            use fs_ntfs::facade::Filesystem;
            let fs = Filesystem::mount_rw(&route.driver_device).map_err(|e| e.to_string())?;
            if folder {
                fs.mkdir(&clean_path(&format!("/{rel}")), "")
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            } else {
                fs.create_file("/".trim_end_matches('/'), &rel)
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
        FsKind::Apfs | FsKind::Hfs | FsKind::Btrfs => Err("该文件系统内置驱动为只读".into()),
    }
}

pub fn remove(route: &Route) -> Result<(), String> {
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
        FsKind::Apfs | FsKind::Hfs | FsKind::Btrfs => Err("该文件系统内置驱动为只读".into()),
    }
}

pub fn rename(route: &Route, new_name: &str) -> Result<(), String> {
    ensure_access(route)?;
    if new_name.contains('/') || new_name.contains('\\') || new_name == "." || new_name == ".." {
        return Err("无效文件名".into());
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
        FsKind::Apfs | FsKind::Hfs | FsKind::Btrfs => Err("该文件系统内置驱动为只读".into()),
    }
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

fn exists(route: &Route) -> bool {
    entry(route).is_ok()
}

fn unique(route: &Route, name: &str) -> String {
    if !exists(&Route {
        rel: join_rel(&route.rel, name),
        ..route.clone()
    }) {
        return name.into();
    }
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.into());
    let ext = path
        .extension()
        .map(|value| format!(".{}", value.to_string_lossy()))
        .unwrap_or_default();
    for index in 2.. {
        let candidate = format!("{stem} ({index}){ext}");
        if !exists(&Route {
            rel: join_rel(&route.rel, &candidate),
            ..route.clone()
        }) {
            return candidate;
        }
    }
    unreachable!()
}

pub fn copy_local_into(
    directory: &Route,
    source: &Path,
    keep_both: bool,
    moving: bool,
) -> Result<String, String> {
    let name = source
        .file_name()
        .ok_or("不能复制根目录")?
        .to_string_lossy()
        .into_owned();
    let name = if keep_both {
        unique(directory, &name)
    } else {
        name
    };
    let (copied, path) = copy_one(directory, source, &name)?;
    if moving && copied {
        if source.is_dir() {
            std::fs::remove_dir_all(source).map_err(|error| error.to_string())?;
        } else {
            std::fs::remove_file(source).map_err(|error| error.to_string())?;
        }
    }
    Ok(path)
}

fn copy_one(directory: &Route, source: &Path, name: &str) -> Result<(bool, String), String> {
    let meta = std::fs::symlink_metadata(source).map_err(|error| error.to_string())?;
    if meta.file_type().is_symlink() {
        return Err("不复制符号链接".into());
    }
    let child = Route {
        rel: join_rel(&directory.rel, name),
        ..directory.clone()
    };
    if meta.is_dir() {
        create(directory, name, true)?;
        for item in std::fs::read_dir(source).map_err(|error| error.to_string())? {
            let item = item.map_err(|error| error.to_string())?;
            let child_name = item.file_name().to_string_lossy().into_owned();
            copy_one(&child, &item.path(), &child_name)?;
        }
    } else if meta.is_file() {
        let data = std::fs::read(source).map_err(|error| error.to_string())?;
        write_path(&child, &data)?;
    } else {
        return Err("不能复制特殊文件".into());
    }
    Ok((
        true,
        format!("{}/{}", directory.root.trim_end_matches('/'), child.rel),
    ))
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

fn watchers() -> &'static std::sync::Mutex<Vec<notify::RecommendedWatcher>> {
    static WATCHERS: std::sync::OnceLock<std::sync::Mutex<Vec<notify::RecommendedWatcher>>> =
        std::sync::OnceLock::new();
    WATCHERS.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

/// Extracts a file to the local cache and watches it, writing edits back to the
/// raw filesystem whenever the external application saves.
pub fn open_cached(route: &Route) -> Result<String, String> {
    let data = read_path(route)?;
    let cache = dirs::home_dir()
        .ok_or("无法创建缓存目录")?
        .join(".fiex")
        .join("usbfs-cache")
        .join(Path::new(&route.root).file_name().unwrap_or_default())
        .join(&route.rel);
    if let Some(parent) = cache.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(&cache, data).map_err(|error| error.to_string())?;
    if matches!(route.kind, FsKind::Apfs | FsKind::Hfs | FsKind::Btrfs) {
        return Ok(cache.to_string_lossy().into_owned());
    }
    fn digest(data: &[u8]) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        data.hash(&mut hasher);
        hasher.finish()
    }
    let watched = route.clone();
    let watched_file = cache.clone();
    let original = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(digest(&read_path(
        route,
    )?)));
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        let Ok(event) = event else { return };
        if !matches!(
            event.kind,
            notify::EventKind::Modify(_) | notify::EventKind::Create(_)
        ) {
            return;
        }
        if !event.paths.iter().any(|path| path == &watched_file) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(220));
        let Ok(data) = std::fs::read(&watched_file) else {
            return;
        };
        let Ok(current) = read_path(&watched) else {
            return;
        };
        if digest(&current) != original.load(std::sync::atomic::Ordering::SeqCst) {
            let conflict = dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".fiex")
                .join("usbfs-conflicts")
                .join(format!(
                    "{}-{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|value| value.as_millis())
                        .unwrap_or_default(),
                    watched_file
                        .file_name()
                        .map(|value| value.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "file".into())
                ));
            if let Some(parent) = conflict.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(conflict, &data);
            return;
        }
        if write_path(&watched, &data).is_ok() {
            original.store(digest(&data), std::sync::atomic::Ordering::SeqCst);
        }
    })
    .map_err(|error| error.to_string())?;
    if let Some(parent) = cache.parent() {
        watcher
            .watch(parent, notify::RecursiveMode::NonRecursive)
            .map_err(|error| error.to_string())?;
    }
    watchers().lock().unwrap().push(watcher);
    Ok(cache.to_string_lossy().into_owned())
}

pub fn eject(device: &str) -> Result<(), String> {
    authorized().lock().unwrap().remove(device);
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("diskutil")
            .args(["eject", device])
            .output()
            .map_err(|error| error.to_string())?;
        if output.status.success() {
            return Ok(());
        }
        Err(String::from_utf8_lossy(&output.stderr).trim().into())
    }
    #[cfg(target_os = "linux")]
    {
        let output = std::process::Command::new("udisksctl")
            .args(["power-off", "-b", device])
            .output()
            .map_err(|error| error.to_string())?;
        if output.status.success() {
            return Ok(());
        }
        let output = std::process::Command::new("eject")
            .arg(device)
            .output()
            .map_err(|error| error.to_string())?;
        if output.status.success() {
            return Ok(());
        }
        Err("无法弹出设备".into())
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = device;
        Err("当前系统请通过系统托盘弹出设备".into())
    }
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
