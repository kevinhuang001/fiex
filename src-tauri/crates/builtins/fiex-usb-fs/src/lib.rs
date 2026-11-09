//! Read-only filesystem drivers for USB volumes without a native OS driver.
//!
//! Device discovery, mount identity, access serialization and filesystem
//! parsing live here. Core adapts these filesystem entries to its provider
//! entry model and owns authorization, presentation and OS ejection policy.

pub const PROVIDER_ID: &str = "core:usb-fs";

#[cfg(feature = "driver")]
mod access;
#[cfg(feature = "driver")]
mod identity;
#[cfg(feature = "driver")]
mod mount;
#[cfg(feature = "driver")]
mod tree;
#[cfg(feature = "driver")]
mod util;

#[cfg(feature = "driver")]
pub use identity::{generation, invalidate_generation};
#[cfg(feature = "driver")]
pub use mount::{detect, ensure_access, probe};
#[cfg(feature = "driver")]
pub use tree::{copy_file_to, list, read, read_at};
#[cfg(feature = "driver")]
pub use util::{FsKind, Route, UsbEntry, route, validate_child, virtual_root};

#[cfg(all(test, feature = "driver"))]
mod tests {
    use super::*;

    #[test]
    fn fallback_detection_excludes_system_driver_filesystems() {
        assert_eq!(detect("NTFS"), None);
        assert_eq!(detect("ext4"), None);
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
        assert!(ensure_access(&route).is_ok());
        assert!(list(&device, FsKind::Btrfs, "").is_ok());
    }
}
