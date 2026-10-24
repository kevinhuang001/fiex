//! In-process USB filesystem support for NTFS and ext4.
//!
//! The drivers operate on a block device or disk image directly; no OS mount
//! and no FUSE are involved. APFS, HFS+ and btrfs remain read-only elsewhere.

mod mount;
mod tree;
mod util;
mod write;

pub use mount::{detect, eject, ensure_access, probe};
pub use tree::{copy_to_local, entry, list, listing, read, read_path, search};
pub use util::{route, virtual_root, FsKind, Route, UsbEntry};
pub use write::{copy_local_into, create, open_cached, remove, rename, write, write_path};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_supported_filesystems() {
        assert!(detect("NTFS").is_some());
        assert!(detect("ext4").is_some());
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
        assert!(listing(&route).is_ok());
    }

    #[test]
    fn virtual_route_end_to_end() {
        use fs_ntfs::block_io::{BlockIo, PathIo};
        use std::fs::File;
        let temp = tempfile::tempdir().unwrap();
        let image = temp.path().join("virtual-usb.img");
        let file = File::create(&image).unwrap();
        file.set_len(64 * 1024 * 1024).unwrap();
        drop(file);
        let mut io = PathIo::open_rw(&image).unwrap();
        fs_ntfs::mkfs::format_filesystem(
            &mut io,
            64 * 1024 * 1024,
            4096,
            4096,
            Some("USB"),
            Some(1),
        )
        .unwrap();
        io.sync().unwrap();
        drop(io);
        let device = image.to_string_lossy().into_owned();
        assert_eq!(probe(&device), Some(FsKind::Ntfs));
        let root = virtual_root(&device, FsKind::Ntfs);
        let root_route = route(&root).expect("virtual root route");
        assert_eq!(root_route.device, device);
        create(&root_route, "notes.txt", false).unwrap();
        let file_path = format!("{root}/notes.txt");
        let file_route = route(&file_path).expect("file route");
        write_path(&file_route, b"virtual usb").unwrap();
        assert_eq!(read_path(&file_route).unwrap(), b"virtual usb");
        let entries = listing(&root_route).unwrap();
        assert!(entries.iter().any(|entry| entry.path == file_path));
        assert!(search(&root_route, "notes", 10)
            .unwrap()
            .iter()
            .any(|entry| entry.name == "notes.txt"));
        let local = temp.path().join("local");
        std::fs::create_dir(&local).unwrap();
        copy_to_local(&file_route, &local, false, false).unwrap();
        assert_eq!(
            std::fs::read(local.join("notes.txt")).unwrap(),
            b"virtual usb"
        );
    }

    #[test]
    fn ext4_image_roundtrip() {
        use fs_ext4::block_io::FileDevice;
        use std::fs::File;
        let temp = tempfile::tempdir().unwrap();
        let image = temp.path().join("usb-ext4.img");
        let file = File::create(&image).unwrap();
        file.set_len(64 * 1024 * 1024).unwrap();
        drop(file);
        let device = image.to_string_lossy().into_owned();
        let dev = FileDevice::open_rw(&device).unwrap();
        fs_ext4::mkfs::format_filesystem(&dev, Some("FIEX"), None, 64 * 1024 * 1024, 4096).unwrap();
        drop(dev);
        let root = Route {
            device: device.clone(),
            driver_device: device.clone(),
            kind: FsKind::Ext4,
            rel: String::new(),
            root: virtual_root(&device, FsKind::Ext4),
        };
        create(&root, "hello.txt", false).unwrap();
        let file = Route {
            rel: "hello.txt".into(),
            ..root.clone()
        };
        write_path(&file, b"hello ext4").unwrap();
        assert_eq!(read_path(&file).unwrap(), b"hello ext4");
        assert!(listing(&root)
            .unwrap()
            .iter()
            .any(|entry| entry.name == "hello.txt"));
        rename(&file, "renamed.txt").unwrap();
        let renamed = Route {
            rel: "renamed.txt".into(),
            ..root.clone()
        };
        assert_eq!(read_path(&renamed).unwrap(), b"hello ext4");
        remove(&renamed).unwrap();
    }

    #[test]
    fn ntfs_image_roundtrip() {
        use fs_ntfs::block_io::{BlockIo, PathIo};
        use std::fs::File;
        let temp = tempfile::tempdir().unwrap();
        let image = temp.path().join("usb.img");
        let file = File::create(&image).unwrap();
        file.set_len(64 * 1024 * 1024).unwrap();
        drop(file);
        let mut io = PathIo::open_rw(&image).unwrap();
        fs_ntfs::mkfs::format_filesystem(
            &mut io,
            64 * 1024 * 1024,
            4096,
            4096,
            Some("FIEX"),
            Some(1),
        )
        .unwrap();
        io.sync().unwrap();
        drop(io);
        let device = image.to_string_lossy().into_owned();
        let root = Route {
            device: device.clone(),
            driver_device: device.clone(),
            kind: FsKind::Ntfs,
            rel: String::new(),
            root: virtual_root(&device, FsKind::Ntfs),
        };
        create(&root, "hello.txt", false).unwrap();
        let file = Route {
            rel: "hello.txt".into(),
            ..root.clone()
        };
        write_path(&file, b"hello usb").unwrap();
        assert_eq!(read_path(&file).unwrap(), b"hello usb");
        assert!(listing(&root)
            .unwrap()
            .iter()
            .any(|entry| entry.name == "hello.txt"));
        rename(&file, "renamed.txt").unwrap();
        let renamed = Route {
            rel: "renamed.txt".into(),
            ..root.clone()
        };
        assert_eq!(read_path(&renamed).unwrap(), b"hello usb");
        remove(&renamed).unwrap();
    }
}
