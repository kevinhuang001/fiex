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
        macos_put_back_path("System/Volumes/Data/Users/kevin/Desktop", "桌面文件.txt").as_deref(),
        Some("/Users/kevin/Desktop/桌面文件.txt")
    );
}

#[test]
fn rejects_put_back_names_that_are_not_single_path_components() {
    assert!(macos_put_back_path("/Users/kevin", "../outside.txt").is_none());
    assert!(macos_put_back_path("/Users/kevin", "/outside.txt").is_none());
    assert!(macos_put_back_path("/Users/../private", "file.txt").is_none());
    assert!(macos_put_back_path("System/Volumes/Data/Users/./kevin", "file.txt").is_none());
}

#[cfg(not(windows))]
#[test]
fn finder_item_put_back_does_not_request_fallback_without_fiex_journal() {
    use super::{original_location, restore_needs_folder_in};
    use crate::{TRASH_URI, TrashDirectory};

    let temp = tempfile::Builder::new()
        .prefix("fiex-finder-put-back-")
        .tempdir()
        .unwrap();
    let temp_name = temp.path().file_name().unwrap().to_string_lossy();
    let name: String = temp_name
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .rev()
        .take(5)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    assert_eq!(name.chars().count(), 5);
    assert!(crate::journal::original(&name).is_none());

    let mut store = include_bytes!("../../tests/data/putback_types.DS_Store").to_vec();
    let from: Vec<u8> = "a.jpg".encode_utf16().flat_map(u16::to_be_bytes).collect();
    let to: Vec<u8> = name.encode_utf16().flat_map(u16::to_be_bytes).collect();
    assert_eq!(from.len(), to.len());
    let positions: Vec<_> = store
        .windows(from.len())
        .enumerate()
        .filter_map(|(position, window)| (window == from.as_slice()).then_some(position))
        .collect();
    assert!(positions.len() >= 2);
    for position in positions {
        store[position..position + to.len()].copy_from_slice(&to);
    }

    let trashed = temp.path().join(&name);
    std::fs::write(&trashed, b"deleted outside Fiex").unwrap();
    std::fs::write(temp.path().join(".DS_Store"), store).unwrap();
    let expected = format!("/Users/u/Pics/{name}");
    assert_eq!(
        original_location(&trashed).as_deref(),
        Some(expected.as_str())
    );
    let trash = TrashDirectory::with_root(temp.path().to_owned());
    let uri = format!("{TRASH_URI}{name}");
    assert!(!restore_needs_folder_in(&[uri], &trash));
}

#[cfg(not(windows))]
#[test]
fn restores_an_external_finder_trash_item_from_its_ds_store_location() {
    use super::{restore_items_in, restore_needs_folder_in};
    use crate::{TRASH_URI, TrashDirectory};
    use std::{path::Path, sync::atomic::AtomicBool};

    let temp = tempfile::Builder::new()
        .prefix("fiex-external-put-back-")
        .tempdir()
        .unwrap();
    let temp_name = temp.path().file_name().unwrap().to_string_lossy();
    let destination_id: String = temp_name
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let name: String = temp_name
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .rev()
        .take(5)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    assert_eq!(name.chars().count(), 5);
    assert_eq!(destination_id.chars().count(), 8);
    assert!(crate::journal::original(&name).is_none());

    // Keep the Finder path exactly as long as the fixture's original path so
    // the DS_Store B-tree remains structurally unchanged.
    let destination_parent = Path::new("/tmp").join(&destination_id);
    std::fs::create_dir(&destination_parent).unwrap();
    let destination = destination_parent.join(&name);
    let finder_location = format!("System/Volumes/Data/tmp/{destination_id}/");
    let mut store = include_bytes!("../../tests/data/putback_types.DS_Store").to_vec();
    replace_utf16be(&mut store, "a.jpg", &name);
    replace_utf16be(
        &mut store,
        "System/Volumes/Data/Users/u/Pics/",
        &finder_location,
    );

    let trash_root = temp.path().join("trash-files");
    std::fs::create_dir(&trash_root).unwrap();
    let source = trash_root.join(&name);
    std::fs::write(&source, b"deleted outside Fiex").unwrap();
    std::fs::write(trash_root.join(".DS_Store"), store).unwrap();
    let trash = TrashDirectory::with_root(trash_root);
    let uri = format!("{TRASH_URI}{name}");

    assert!(!restore_needs_folder_in(std::slice::from_ref(&uri), &trash));
    assert_eq!(
        restore_items_in(&[uri], None, &AtomicBool::new(false), &trash).unwrap(),
        1
    );
    assert_eq!(
        std::fs::read(&destination).unwrap(),
        b"deleted outside Fiex"
    );
    assert!(!source.exists());

    std::fs::remove_dir_all(destination_parent).unwrap();
}

#[cfg(not(windows))]
fn replace_utf16be(data: &mut [u8], from: &str, to: &str) {
    let from: Vec<u8> = from.encode_utf16().flat_map(u16::to_be_bytes).collect();
    let to: Vec<u8> = to.encode_utf16().flat_map(u16::to_be_bytes).collect();
    assert_eq!(from.len(), to.len());
    let positions: Vec<_> = data
        .windows(from.len())
        .enumerate()
        .filter_map(|(position, window)| (window == from.as_slice()).then_some(position))
        .collect();
    assert!(!positions.is_empty());
    for position in positions {
        data[position..position + to.len()].copy_from_slice(&to);
    }
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

    assert!(!restore_needs_folder(std::slice::from_ref(
        &trashed.node_id
    )));
    assert_eq!(
        restore_from_trash(
            std::slice::from_ref(&trashed.node_id),
            None,
            &AtomicBool::new(false),
        )
        .unwrap(),
        1
    );
    assert_eq!(
        std::fs::read(&original).unwrap(),
        b"restore to this exact path"
    );
}
