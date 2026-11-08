//! Volume discovery: the sidebar root list and the mount table.
//!
//! The implementation is split by concern:
//! - `roots`: recent, home favorites and the applications entry,
//! - `mounts`: mounted disks, unmounted removable media and mounting.
//! - `trash_vfs`: Core presentation glue for the independent `fiex-trash` module.

pub mod application_policy;
pub mod applications;
mod mounts;
pub mod native_vfs;
pub mod recycle;
mod roots;
pub mod trash_vfs;
#[cfg(target_os = "windows")]
pub(crate) mod windows_native;
#[cfg(target_os = "windows")]
pub(crate) mod windows_shell;

use serde::Serialize;

pub use applications as apps;
pub use mounts::{clear_usb_fallback, mount_device, usb_fallback_eject_device};
pub use recycle as recycle_bin;

#[derive(Serialize)]
pub struct Location {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub available: u64,
    pub total: u64,
    pub removable: bool,
    pub filesystem: String,
    pub device: String,
    pub mounted: bool,
    pub usbfs: bool,
}

/// Every location the sidebar offers, in display order: stable roots first,
/// then the current mount table and finally ejected removable media.
pub fn locations() -> Vec<Location> {
    let mut out = roots::base_locations();
    out.extend(mounts::disks());
    out
}
