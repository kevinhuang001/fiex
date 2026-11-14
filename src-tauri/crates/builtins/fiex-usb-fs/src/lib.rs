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

    #[cfg(target_os = "linux")]
    #[test]
    fn raw_access_preserves_linux_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let device = temp.path().join("permission-check.img");
        std::fs::write(&device, b"filesystem image").unwrap();
        let route = Route {
            device: device.to_string_lossy().into_owned(),
            driver_device: device.to_string_lossy().into_owned(),
            kind: FsKind::Btrfs,
            rel: String::new(),
            root: virtual_root(&device.to_string_lossy(), FsKind::Btrfs),
        };

        std::fs::set_permissions(&device, std::fs::Permissions::from_mode(0o400)).unwrap();
        ensure_access(&route).unwrap();
        assert_eq!(
            std::fs::metadata(&device).unwrap().permissions().mode() & 0o777,
            0o400
        );

        std::fs::set_permissions(&device, std::fs::Permissions::from_mode(0o0)).unwrap();
        let result = ensure_access(&route);
        assert_eq!(
            std::fs::metadata(&device).unwrap().permissions().mode() & 0o777,
            0
        );
        if unsafe { libc::geteuid() } != 0 {
            assert!(result.unwrap_err().contains("PermissionDenied"));
        }
    }

    #[cfg(target_os = "linux")]
    fn test_route(device: &str) -> Route {
        Route {
            device: device.to_owned(),
            driver_device: device.to_owned(),
            kind: FsKind::Btrfs,
            rel: String::new(),
            root: virtual_root(device, FsKind::Btrfs),
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn loopback_block_device_uses_existing_os_access_without_chmod() {
        use std::os::unix::fs::{FileTypeExt, PermissionsExt};

        let Ok(device) = std::env::var("FIEX_TEST_RAW_DEVICE") else {
            eprintln!("skipped: FIEX_TEST_RAW_DEVICE is not configured");
            return;
        };
        let metadata = std::fs::metadata(&device).unwrap();
        assert!(metadata.file_type().is_block_device());
        let mode = metadata.permissions().mode() & 0o777;

        ensure_access(&test_route(&device)).unwrap();

        assert_eq!(
            std::fs::metadata(&device).unwrap().permissions().mode() & 0o777,
            mode,
            "raw access must not change the device node permissions"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn mounted_loopback_device_is_refused_without_changing_permissions() {
        use std::os::unix::fs::{FileTypeExt, PermissionsExt};

        let Ok(device) = std::env::var("FIEX_TEST_RAW_DEVICE_MOUNTED") else {
            eprintln!("skipped: FIEX_TEST_RAW_DEVICE_MOUNTED is not configured");
            return;
        };
        let metadata = std::fs::metadata(&device).unwrap();
        assert!(metadata.file_type().is_block_device());
        let mode = metadata.permissions().mode() & 0o777;

        let error = ensure_access(&test_route(&device)).unwrap_err();

        assert!(error.contains("unmount the device"), "{error}");
        assert_eq!(
            std::fs::metadata(&device).unwrap().permissions().mode() & 0o777,
            mode,
            "refusing a mounted device must not change node permissions"
        );
        let lock_error = match access::lock(&device) {
            Ok(_guard) => panic!("mounted raw device must not be locked for fallback access"),
            Err(error) => error,
        };
        assert!(
            lock_error.contains("mounted, busy or not granted"),
            "{lock_error}"
        );
    }
}
