//! Mount table facade: combine mounted volumes with platform removable-media discovery.

mod fallback;
mod platform;
#[cfg(test)]
mod tests;

pub use fallback::{clear_usb_fallback, usb_fallback_eject_device};
pub use platform::mount_device;

use super::Location;
use std::collections::HashSet;

/// Mounted disks plus removable volumes that are currently unmounted.
pub(super) fn disks() -> Vec<Location> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for disk in sysinfo::Disks::new_with_refreshed_list().list() {
        let path = disk.mount_point().to_string_lossy().into_owned();
        #[cfg(target_os = "macos")]
        if !platform::visible_mount(&path) {
            continue;
        }
        let mut name = disk.name().to_string_lossy().into_owned();
        if name.is_empty() {
            name = path.clone();
        }
        if !seen.insert((name.clone(), disk.total_space())) {
            continue;
        }
        out.push(Location {
            name,
            path,
            kind: "disk".into(),
            available: disk.available_space(),
            total: disk.total_space(),
            removable: disk.is_removable(),
            filesystem: disk.file_system().to_string_lossy().into_owned(),
            device: String::new(),
            mounted: true,
            usbfs: false,
        });
    }
    out.extend(platform::unmounted_removable());
    out
}
