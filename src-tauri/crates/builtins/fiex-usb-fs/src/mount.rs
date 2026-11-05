//! Device discovery, filesystem driver mounting, access elevation and ejection.

use super::util::{FsKind, Route};

pub fn probe(device: &str) -> Option<FsKind> {
    use std::io::Read;
    let mut file = std::fs::File::open(device).ok()?;
    let mut head = vec![0u8; 70_000];
    let read = file.read(&mut head).ok()?;
    head.truncate(read);
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
        "apfs" => Some(FsKind::Apfs),
        "btrfs" => Some(FsKind::Btrfs),
        "hfs" | "hfs+" | "hfsplus" | "hfsx" | "mac os extended" | "journaled hfs+" => {
            Some(FsKind::Hfs)
        }
        _ => None,
    }
}

pub(crate) fn btrfs_fs(device: &str) -> Result<fs_btrfs::Filesystem, String> {
    use fs_ext4::block_io::FileDevice;
    let device = FileDevice::open(device).map_err(|error| error.to_string())?;
    fs_btrfs::Filesystem::mount(std::sync::Arc::new(device)).map_err(|error| error.to_string())
}

/// Never change device-node permissions. Raw access requires an existing OS
/// grant and an unmounted device; disk images remain usable for driver tests.
pub fn ensure_access(route: &Route) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    if route.driver_device.starts_with(r"\\.\") {
        std::fs::File::open(&route.driver_device).map_err(|_| {
            "PermissionDenied: raw volume access has not been granted by Windows".to_string()
        })?;
        return Ok(());
    }
    let metadata = std::fs::metadata(&route.driver_device).map_err(|e| e.to_string())?;
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::{FileTypeExt, MetadataExt};
        if metadata.file_type().is_block_device() {
            let device = format!(
                "{}:{}",
                libc::major(metadata.rdev()),
                libc::minor(metadata.rdev())
            );
            let mounts =
                std::fs::read_to_string("/proc/self/mountinfo").map_err(|e| e.to_string())?;
            if mounts
                .lines()
                .any(|line| line.split_whitespace().nth(2) == Some(device.as_str()))
            {
                return Err(
                    "PermissionDenied: unmount the device before raw filesystem access".into(),
                );
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::fs::FileTypeExt;
        if !metadata.is_file()
            && !metadata.file_type().is_block_device()
            && !metadata.file_type().is_char_device()
        {
            return Err("PermissionDenied: raw devices require an OS-granted device handle".into());
        }
    }
    #[cfg(all(not(target_os = "linux"), not(target_os = "macos")))]
    if !metadata.is_file() {
        return Err("PermissionDenied: raw devices require a scoped platform device broker".into());
    }
    std::fs::File::open(&route.driver_device).map_err(|_| {
        "PermissionDenied: raw device access has not been granted by the OS".to_string()
    })?;
    Ok(())
}
