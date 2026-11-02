//! Read-only provider fallbacks for Btrfs, APFS and HFS+ volumes without an
//! available system filesystem driver. Supported native filesystems use the
//! system mount and the regular Local FS provider.

mod access;
mod identity;
mod mount;
pub use identity::generation;
mod tree;
mod util;

pub use mount::{detect, eject, ensure_access, probe};
pub use tree::{
    copy_file_to, copy_to_local, entry, list, listing, read, read_at, read_path, search,
};
pub use util::{route, validate_child, virtual_root, FsKind, Route, UsbEntry};

#[cfg(test)]
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
        let root = entry(&route).unwrap();
        for operation in ["write", "mkdir", "rename", "remove"] {
            assert!(!root.allowed_ops.iter().any(|allowed| allowed == operation));
        }
        assert!(listing(&route).is_ok());
    }
}
