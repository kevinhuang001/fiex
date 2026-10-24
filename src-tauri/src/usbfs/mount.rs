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
        Err("当前系统需要通过管理员权限访问原始设备".into())
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
