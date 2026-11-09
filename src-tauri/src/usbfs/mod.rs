//! Core adapter for the read-only USB filesystem driver crate.

mod tree;

pub use fiex_usb_fs::{
    FsKind, Route, UsbEntry, copy_file_to, detect, ensure_access, generation,
    invalidate_generation, list, probe, read, read_at, route, validate_child, virtual_root,
};
pub use tree::{copy_to_local, entry, listing, read_path, search};

/// Ejection remains in the platform composition layer; the driver crate only
/// owns the resource generation invalidation and filesystem access boundary.
pub fn eject(device: &str) -> Result<(), String> {
    invalidate_generation(device);
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
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::eject_path(device)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = device;
        Err(crate::i18n::t("当前系统请通过系统托盘弹出设备").into())
    }
}
