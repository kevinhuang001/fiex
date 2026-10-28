//! Volume discovery: the sidebar root list, the mount table and the platform
//! Trash.
//!
//! The implementation is split by concern:
//! - `roots`: recent, home favorites and the applications entry,
//! - `mounts`: mounted disks, unmounted removable media and mounting,
//! - `trash`: browsing, restoring and emptying the Trash.

pub mod applications;
mod mounts;
pub mod recycle;
mod roots;
mod trash;
pub mod virtual_resolvers;
#[cfg(target_os = "windows")]
mod windows_shell;

use serde::Serialize;

pub use applications as apps;
pub use mounts::mount_device;
pub use recycle as recycle_bin;
pub use trash::{
    delete_permanently, empty_trash, record_trashed, restore_from_trash, restore_needs_folder,
    trash_names, trash_path,
};

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
