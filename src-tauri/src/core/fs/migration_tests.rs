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
    assert_eq!(error.kind, FsErrorKind::StaleRef, "{error}");
    assert!(error.to_string().starts_with("StaleRef:"), "{error}");
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

#[test]
fn feature_discovery_receives_independent_operation_capabilities() {
    use crate::plugins::resource::{
        file_capabilities::FileCapabilities, resource_context::ResourceSnapshot,
    };
    let mut resource = ResourceSnapshot::from_metadata(
        "example",
        crate::plugins::contracts::ResourceMetadata {
            name: "example.txt".into(),
            kind: "file".into(),
            ..Default::default()
        },
    );
    resource.operations = Some(FileCapabilities {
        stat: true,
        ..Default::default()
    });
    let rules: crate::plugins::match_rules::MatchRules =
        serde_json::from_value(json!({"requiredOps":["read"]})).unwrap();
    assert!(!rules.matches_event(
        &crate::plugins::events::resource_metadata(&[resource.clone()]),
        ""
    ));
    resource.operations.as_mut().unwrap().read = true;
    assert!(rules.matches_event(&crate::plugins::events::resource_metadata(&[resource]), ""));
}

#[cfg(unix)]
#[test]
fn symlink_kind_survives_event_metadata_projection() {
    use crate::plugins::resource::resource_context::ResourceSnapshot;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("file");
    let link = dir.path().join("link");
    std::fs::write(&file, b"bytes").unwrap();
    std::os::unix::fs::symlink(&file, &link).unwrap();
    let resource = ResourceSnapshot::resolve(&link.to_string_lossy()).unwrap();
    assert_eq!(
        crate::plugins::events::resource_metadata(&[resource])[0]["kind"],
        "symlink"
    );
}

#[test]
fn mixed_native_and_virtual_exports_preserve_order_and_duplicate_names() {
    use crate::core::volumes::applications;
    let local = tempfile::tempdir().unwrap();
    let native = local.path().join("same.txt");
    std::fs::write(&native, b"local").unwrap();
    let token = crate::plugins::mounts::new_id().unwrap();
    let mut directories = Vec::new();
    let mut inputs = vec![native.to_string_lossy().into_owned()];
    for index in 0..2 {
        let dir = applications::create_stored(
            applications::URI,
            &format!("export-{token}-{index}"),
            true,
        )
        .unwrap();
        let item = applications::create_stored(&dir, "same.txt", false).unwrap();
        std::fs::write(
            applications::candidate_path(&item).unwrap(),
            index.to_string(),
        )
        .unwrap();
        inputs.push(item);
        directories.push(dir);
    }
    let output = export_for_external_drag(&inputs).unwrap();
    assert_eq!(output.len(), 3);
    assert_eq!(std::fs::read(&output[0]).unwrap(), b"local");
    assert_eq!(std::fs::read(&output[1]).unwrap(), b"0");
    assert_eq!(std::fs::read(&output[2]).unwrap(), b"1");
    assert_ne!(output[1], output[2]);
    for path in &output {
        assert_eq!(std::path::Path::new(path).file_name().unwrap(), "same.txt");
    }
    for directory in directories {
        applications::remove_user(&directory).unwrap();
    }
}

#[test]
fn selected_local_path_cannot_be_relabelled_with_a_forged_reference() {
    use crate::plugins::resource::resource_context::{
        bind_selected, ResourceBinding, ResourceSnapshot,
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("selected.txt");
    std::fs::write(&path, b"selected").unwrap();
    let mut scope = Scope::selected(dir.path(), std::slice::from_ref(&path)).unwrap();
    let mut reference = ResourceSnapshot::new(&path.to_string_lossy(), &path).reference;
    reference.mount_id = "forged-mount".into();
    assert!(bind_selected(
        &mut scope,
        &[ResourceBinding {
            reference,
            path: path.to_string_lossy().into_owned()
        }]
    )
    .is_err());
}

#[test]
fn desktop_create_rename_and_delete_use_the_owning_adapter() {
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
    let name = format!(
        "fiex-broker-{}.txt",
        crate::plugins::mounts::new_id().unwrap()
    );
    let item =
        super::binding::create(applications::URI, &name, false, "core:test-app-create").unwrap();
    assert!(applications::is_user(&item));
    let renamed =
        super::binding::rename_path(&item, &format!("{name}.moved"), "core:test-app-rename")
            .unwrap();
    assert!(applications::is_user(&renamed));
    super::binding::remove_path(&renamed, "core:test-app-delete").unwrap();
    assert!(applications::stored_entry(&renamed).is_err());
}

#[test]
fn desktop_search_runs_through_the_same_adapter_and_honors_cancellation() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("nested")).unwrap();
    std::fs::write(dir.path().join("nested").join("needle.txt"), b"x").unwrap();
    let root = super::binding::trusted(&dir.path().to_string_lossy(), "core:test-search").unwrap();
    let (entries, truncated, skipped) = root.search("needle", false, &[], 50, &|| false).unwrap();
    assert!(!truncated);
    assert_eq!(skipped, 0);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "needle.txt");
    let (entries, _, _) = root.search("needle", false, &[], 50, &|| true).unwrap();
    assert!(entries.is_empty());
}

#[test]
fn builtin_view_storage_and_presentation_are_separate() {
    use crate::core::volumes::{applications, virtual_resolvers, virtual_storage};
    let root = virtual_storage::entry(applications::URI).unwrap();
    assert!(root.is_dir);
    assert_eq!(root.name, "core:applications");
    assert!(root.allowed_ops.iter().any(|op| op == "mkdir"));
    assert_eq!(
        virtual_resolvers::view_for(applications::URI)
            .map(|view| view.id())
            .unwrap(),
        "core:applications"
    );
    assert!(virtual_storage::list(virtual_resolvers::RECENT_URI).is_ok());
}
