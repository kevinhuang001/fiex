use super::*;

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
#[cfg(not(target_os = "macos"))]
#[test]
fn mixed_native_and_virtual_exports_preserve_order_and_duplicate_names() {
    use crate::core::volumes::applications;
    let local = tempfile::tempdir().unwrap();
    let native = local.path().join("same.txt");
    std::fs::write(&native, b"local").unwrap();
    let token = crate::plugins::mounts::new_id().unwrap();
    let mut directories = Vec::new();
    let mut virtual_files = Vec::new();
    let mut inputs = vec![native.to_string_lossy().into_owned()];
    let export_name = if cfg!(target_os = "windows") {
        "same.exe"
    } else {
        "same.desktop"
    };
    let mut expected = vec![b"local".to_vec()];
    for index in 0..2 {
        let dir = applications::create_virtual_folder(
            applications::URI,
            &format!("export-{token}-{index}"),
        )
        .unwrap()
        .path;
        let item = applications::reserve_child(&dir, export_name, false).unwrap();
        let contents = if cfg!(target_os = "windows") {
            vec![b'M', b'Z', index as u8]
        } else {
            index.to_string().into_bytes()
        };
        let item_path = applications::asset_path(&item).unwrap();
        std::fs::write(&item_path, &contents).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&item_path).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&item_path, permissions).unwrap();
        }
        applications::commit_reserved_application(&item).unwrap();
        inputs.push(item);
        virtual_files.push(inputs.last().unwrap().clone());
        directories.push(dir);
        expected.push(contents);
    }
    let output = export_for_external_drag(
        &inputs,
        crate::core::fs::mapped_access::UserActionContext::for_test(),
    )
    .unwrap();
    assert_eq!(output.len(), 3);
    for (path, bytes) in output.iter().zip(expected) {
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
    assert_ne!(output[1], output[2]);
    assert_eq!(
        std::path::Path::new(&output[0]).file_name().unwrap(),
        "same.txt"
    );
    for path in &output[1..] {
        assert_eq!(std::path::Path::new(path).file_name().unwrap(), export_name);
    }
    for file in virtual_files {
        applications::remove_node(&file).unwrap();
    }
    for directory in directories {
        applications::remove_virtual_folder(&directory).unwrap();
    }
}
