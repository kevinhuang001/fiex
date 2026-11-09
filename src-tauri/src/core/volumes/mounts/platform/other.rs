//! Fallbacks for targets without native removable volume support.
use super::super::super::Location;

#[cfg(any(
    not(any(target_os = "macos", target_os = "linux", windows)),
    all(windows, not(feature = "usbfs"))
))]
pub(crate) fn unmounted_removable() -> Vec<Location> {
    Vec::new()
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
pub fn mount_device(_device: &str) -> Result<String, String> {
    Err(crate::i18n::t("当前系统请通过磁盘管理挂载该设备").into())
}
