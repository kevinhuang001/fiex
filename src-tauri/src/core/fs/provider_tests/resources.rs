use super::*;

#[test]
fn usb_replacement_revokes_existing_broker_before_driver_access() {
    let dir = tempfile::tempdir().unwrap();
    let image = dir.path().join("device.img");
    std::fs::write(&image, b"first device").unwrap();
    let root = crate::usbfs::virtual_root(&image.to_string_lossy(), crate::usbfs::FsKind::Btrfs);
    let snapshot =
        crate::plugins::resource::resource_context::from_metadata(&root, Default::default());
    let target = json!({"ref":snapshot.reference});
    let mut scope = Scope::selected(dir.path(), &[]).unwrap();
    scope.bind_remote(&target, &snapshot).unwrap();
    let broker = ResourceBroker::resolve(&scope, "core:test-usb", &target["ref"]).unwrap();
    std::fs::rename(&image, dir.path().join("detached.img")).unwrap();
    std::fs::write(&image, b"second device").unwrap();
    let error = broker.require(FsOperation::Read, false).unwrap_err();
    assert_eq!(error.kind, FsErrorKind::StaleRef, "{error}");
    assert!(error.to_string().starts_with("StaleRef:"), "{error}");
    assert_ne!(
        snapshot.reference,
        crate::plugins::resource::resource_context::from_metadata(&root, Default::default())
            .reference
    );
}
#[test]
fn selected_local_path_cannot_be_relabelled_with_a_forged_reference() {
    use crate::plugins::resource::resource_context::{ResourceBinding, bind_selected};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("selected.txt");
    std::fs::write(&path, b"selected").unwrap();
    let mut scope = Scope::selected(dir.path(), std::slice::from_ref(&path)).unwrap();
    let mut reference = crate::plugins::resource::resource_context::new_resource_snapshot(
        &path.to_string_lossy(),
        &path,
    )
    .reference;
    reference.mount_id = "forged-mount".into();
    assert!(
        bind_selected(
            &mut scope,
            &[ResourceBinding {
                reference,
                path: path.to_string_lossy().into_owned()
            }]
        )
        .is_err()
    );
}
#[test]
fn desktop_create_rename_and_delete_use_the_owning_provider() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_string_lossy().into_owned();
    let folder = super::binding::create(&root, "folder", true, "core:test-create").unwrap();
    assert!(std::path::Path::new(&folder).is_dir());
    let file = super::binding::create(&folder, "note.txt", false, "core:test-create").unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), b"");
    let renamed = super::binding::rename_path(&file, "renamed.txt", "core:test-rename").unwrap();
    assert!(!std::path::Path::new(&file).exists());
    assert!(std::path::Path::new(&renamed).is_file());
    super::binding::remove_path(&renamed, "core:test-delete").unwrap();
    assert!(!std::path::Path::new(&renamed).exists());
}
#[test]
fn applications_root_creation_uses_the_same_broker_contract() {
    use crate::core::volumes::applications;
    let file = super::binding::create(
        applications::URI,
        "unsupported.txt",
        false,
        "core:test-app-create",
    );
    assert!(file.is_err());
    let folder_name = format!(
        "fiex-broker-dir-{}",
        crate::plugins::mounts::new_id().unwrap()
    );
    let folder = super::binding::create(
        applications::URI,
        &folder_name,
        true,
        "core:test-app-create",
    )
    .unwrap();
    let child = super::binding::create(&folder, "nested.txt", false, "core:test-app-create");
    assert!(child.is_err());
    let renamed = super::binding::rename_path(
        &folder,
        &format!("{folder_name}.moved"),
        "core:test-app-rename",
    )
    .unwrap();
    assert!(applications::is_folder(&renamed));
    super::binding::remove_path(&renamed, "core:test-app-delete").unwrap();
    assert!(applications::entry_for_uri(&renamed).is_err());
}
