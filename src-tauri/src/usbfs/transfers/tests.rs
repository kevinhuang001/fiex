use super::*;
use crate::core::transfers::NEVER_CANCELLED;
#[test]
fn image_transfers_conflicts_and_moves_verify_actual_bytes() {
    use fs_ntfs::block_io::{BlockIo, PathIo};
    let t = tempfile::tempdir().unwrap();
    let image = t.path().join("ntfs.img");
    fs::File::create(&image)
        .unwrap()
        .set_len(64 * 1024 * 1024)
        .unwrap();
    let mut io = PathIo::open_rw(&image).unwrap();
    fs_ntfs::mkfs::format_filesystem(&mut io, 64 * 1024 * 1024, 4096, 4096, Some("FIEX"), Some(1))
        .unwrap();
    io.sync().unwrap();
    drop(io);
    let root = usbfs::virtual_root(image.to_str().unwrap(), usbfs::FsKind::Ntfs);
    let local = t.path().join("source.txt");
    fs::write(&local, "first").unwrap();
    let paths = vec![local.to_string_lossy().into_owned()];
    let result = transfer(&paths, &root, false, false, &[], &[], &NEVER_CANCELLED).unwrap();
    assert!(result.errors.is_empty());
    let in_image = join(&root, "source.txt");
    assert_eq!(read(&in_image).unwrap(), b"first");
    fs::write(&local, "updated").unwrap();
    let conflict = transfer(&paths, &root, false, false, &[], &[], &NEVER_CANCELLED).unwrap();
    assert_eq!(conflict.conflicts.len(), 1);
    assert_eq!(read(&in_image).unwrap(), b"first");
    let choices = vec![Resolution {
        key: conflict.conflicts[0].key.clone(),
        action: Action::Replace,
    }];
    transfer(&paths, &root, false, false, &[], &choices, &NEVER_CANCELLED).unwrap();
    assert_eq!(read(&in_image).unwrap(), b"updated");
    let export = t.path().join("export");
    fs::create_dir(&export).unwrap();
    transfer(
        std::slice::from_ref(&in_image),
        export.to_str().unwrap(),
        true,
        false,
        &[],
        &[],
        &NEVER_CANCELLED,
    )
    .unwrap();
    assert_eq!(fs::read(export.join("source.txt")).unwrap(), b"updated");
    assert!(info(&in_image).is_err());
    assert!(local.exists());
}
#[test]
fn readonly_driver_destinations_are_rejected() {
    for kind in [
        usbfs::FsKind::Apfs,
        usbfs::FsKind::Hfs,
        usbfs::FsKind::Btrfs,
    ] {
        let path = usbfs::virtual_root("/tmp/nonexistent-ci.img", kind);
        assert!(writable(&path).is_err());
    }
}
