use super::macos_put_back_path;

#[test]
fn reconstructs_finder_put_back_path_from_parent_and_original_name() {
    assert_eq!(
        macos_put_back_path("/Users/kevin/Downloads/", "Report.pdf").as_deref(),
        Some("/Users/kevin/Downloads/Report.pdf")
    );
}

#[test]
fn normalizes_apfs_data_volume_firmlink_before_restoring() {
    assert_eq!(
        macos_put_back_path("System/Volumes/Data/Users/kevin/Desktop", "桌面文件.txt")
            .as_deref(),
        Some("/Users/kevin/Desktop/桌面文件.txt")
    );
}

#[test]
fn rejects_put_back_names_that_are_not_single_path_components() {
    assert!(macos_put_back_path("/Users/kevin", "../outside.txt").is_none());
    assert!(macos_put_back_path("/Users/kevin", "/outside.txt").is_none());
}

#[cfg(target_os = "macos")]
#[test]
fn fiex_trashed_item_restores_to_its_original_path_without_fallback() {
    use super::{move_to_trash, restore_from_trash, restore_needs_folder};
    use crate::{TRASH_URI, system_trash_directory};
    use std::sync::atomic::AtomicBool;

    let home = dirs::home_dir().expect("CI user home exists");
    let temp = tempfile::Builder::new()
        .prefix("fiex-put-back-ci-")
        .tempdir_in(home)
        .expect("create test folder on the home volume");
    let test_folder = temp.path().file_name().unwrap().to_string_lossy();
    let original = temp.path().join(format!("restore-{test_folder}.txt"));
    std::fs::write(&original, b"restore to this exact path").unwrap();
    let name = original.file_name().unwrap().to_string_lossy().into_owned();

    move_to_trash(&original).expect("move the test file to macOS Trash");
    let trashed = system_trash_directory()
        .list(TRASH_URI)
        .expect("enumerate macOS Trash")
        .entries
        .into_iter()
        .find(|entry| entry.name == name)
        .expect("find the test item in Trash");

    assert!(!restore_needs_folder(std::slice::from_ref(&trashed.node_id)));
    assert_eq!(
        restore_from_trash(
            std::slice::from_ref(&trashed.node_id),
            None,
            &AtomicBool::new(false),
        )
        .unwrap(),
        1
    );
    assert_eq!(std::fs::read(&original).unwrap(), b"restore to this exact path");
}
