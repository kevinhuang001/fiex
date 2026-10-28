use super::*;
use serde_json::json;
use std::sync::atomic::AtomicBool;

#[test]
fn desktop_listing_and_info_share_broker_and_preserve_projection() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("visible.txt"), b"data").unwrap();
    std::fs::write(dir.path().join(".hidden"), b"hidden").unwrap();
    let broker = trusted(&dir.path().to_string_lossy(), "core:test-list").unwrap();
    let listing = broker.desktop_listing(false, &[]).unwrap();
    assert_eq!(listing.entries.len(), 1);
    assert_eq!(listing.entries[0].name, "visible.txt");
    assert!(!listing.virtual_volume);
    let item = trusted(&listing.entries[0].path, "core:test-stat")
        .unwrap()
        .desktop_entry()
        .unwrap();
    assert_eq!(item.size, 4);
    assert_eq!(item.mime, "text/plain");
    assert_eq!(broker.desktop_listing(true, &[]).unwrap().entries.len(), 2);
}

#[test]
fn filesystem_root_can_be_captured_as_directory() {
    let dir = std::env::temp_dir();
    let root = dir.ancestors().last().unwrap();
    let resource = trusted(&root.to_string_lossy(), "core:test-root").unwrap();
    assert!(resource.desktop_entry().unwrap().is_dir);
}

#[test]
fn adapter_symlink_projection_is_not_lost() {
    let item = super::presentation::entry(json!({"name":"link", "symlink":true})).unwrap();
    assert!(item.is_symlink);
}

#[test]
fn usb_replacement_revokes_existing_broker_before_driver_access() {
    use crate::plugins::resource::resource_context::ResourceSnapshot;
    let dir = tempfile::tempdir().unwrap();
    let image = dir.path().join("device.img");
    std::fs::write(&image, b"first device").unwrap();
    let root = crate::usbfs::virtual_root(&image.to_string_lossy(), crate::usbfs::FsKind::Ntfs);
    let snapshot = ResourceSnapshot::from_metadata(&root, Default::default());
    let target = json!({"ref":snapshot.reference});
    let mut scope = Scope::selected(dir.path(), &[]).unwrap();
    scope.bind_remote(&target, &snapshot).unwrap();
    let broker = ResourceBroker::resolve(&scope, "core:test-usb", &target["ref"]).unwrap();
    std::fs::rename(&image, dir.path().join("detached.img")).unwrap();
    std::fs::write(&image, b"second device").unwrap();
    let error = broker.require(FsOperation::Read, false).unwrap_err();
    assert!(error.starts_with("StaleRef:"), "{error}");
    assert_ne!(
        snapshot.reference,
        ResourceSnapshot::from_metadata(&root, Default::default()).reference
    );
}

#[test]
fn move_keeps_a_source_changed_after_the_last_commit() {
    let source = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let file = source.path().join("file.txt");
    std::fs::write(&file, b"before").unwrap();
    let result = transfer(
        &[file.to_string_lossy().into_owned()],
        &target.path().to_string_lossy(),
        true,
        false,
        &[],
        &[],
        &AtomicBool::new(false),
        &|done, total, _| {
            if done == total {
                std::fs::write(&file, b"new source bytes").unwrap();
            }
        },
    )
    .unwrap();
    assert!(!result.errors.is_empty());
    assert_eq!(std::fs::read(file).unwrap(), b"new source bytes");
    assert_eq!(
        std::fs::read(target.path().join("file.txt")).unwrap(),
        b"before"
    );
}

#[test]
fn cancelling_after_the_last_commit_does_not_delete_the_source() {
    use std::sync::atomic::Ordering;
    let source = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let file = source.path().join("file.txt");
    std::fs::write(&file, b"data").unwrap();
    let cancel = AtomicBool::new(false);
    let result = transfer(
        &[file.to_string_lossy().into_owned()],
        &target.path().to_string_lossy(),
        true,
        false,
        &[],
        &[],
        &cancel,
        &|done, total, _| {
            if done == total {
                cancel.store(true, Ordering::SeqCst);
            }
        },
    )
    .unwrap();
    assert!(!result.errors.is_empty());
    assert!(file.exists());
}
