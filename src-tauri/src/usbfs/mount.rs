//! Device discovery, filesystem driver mounting, access elevation and ejection.

use super::util::{FsKind, Route};

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

pub(crate) fn ext4_mount(device: &str, writable: bool) -> Result<fs_ext4::Filesystem, String> {
    use fs_ext4::block_io::FileDevice;
    let device = if writable {
        FileDevice::open_rw(device)
    } else {
        FileDevice::open(device)
    }
    .map_err(|error| error.to_string())?;
    fs_ext4::Filesystem::mount(std::sync::Arc::new(device)).map_err(|error| error.to_string())
}

pub(crate) fn btrfs_fs(device: &str) -> Result<fs_btrfs::Filesystem, String> {
    use fs_ext4::block_io::FileDevice;
    let device = FileDevice::open(device).map_err(|error| error.to_string())?;
    fs_btrfs::Filesystem::mount(std::sync::Arc::new(device)).map_err(|error| error.to_string())
}

/// Never change device-node permissions. Raw access requires an existing OS
/// grant and an unmounted device; disk images remain usable for driver tests.
pub fn ensure_access(route: &Route) -> Result<(), String> {
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
    #[cfg(not(target_os = "linux"))]
    if !metadata.is_file() {
        return Err("PermissionDenied: raw devices require a scoped platform device broker".into());
    }
    std::fs::File::open(&route.driver_device).map_err(|_| {
        "PermissionDenied: raw device access has not been granted by the OS".to_string()
    })?;
    Ok(())
}

pub fn eject(device: &str) -> Result<(), String> {
    // Revoke before asking the OS to eject, including failed ejection: an
    // in-flight operation must not resume with the old device generation.
    super::identity::invalidate(device);
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
        Err(crate::i18n::t("无法弹出设备").into())
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = device;
        Err(crate::i18n::t("当前系统请通过系统托盘弹出设备").into())
    }
}
