//! Platform-specific mount and removable-volume implementations.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
mod other;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::mount_device;
#[cfg(target_os = "macos")]
pub use macos::mount_device;
#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
pub use other::mount_device;
#[cfg(windows)]
pub use windows::mount_device;

#[cfg(target_os = "linux")]
pub(super) use linux::unmounted_removable;
#[cfg(target_os = "macos")]
pub(super) use macos::{unmounted_removable, visible_mount};
#[cfg(any(
    not(any(target_os = "macos", target_os = "linux", windows)),
    all(windows, not(feature = "usbfs"))
))]
pub(super) use other::unmounted_removable;
#[cfg(all(windows, feature = "usbfs"))]
pub(super) use windows::unmounted_removable;
