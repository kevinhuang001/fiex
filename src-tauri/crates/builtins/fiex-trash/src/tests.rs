use super::*;
use std::path::Path;

#[test]
fn trash_uris_reject_absolute_and_traversal_paths() {
    for uri in [
        "trash://../outside",
        "trash://%2E%2E/outside",
        "trash://%2Fetc/passwd",
        "trash://a/%2E%2E/outside",
        "trash://folder%2Ffile",
    ] {
        assert!(relative(uri).is_err());
    }
    assert_eq!(
        relative("trash://folder/file").unwrap(),
        PathBuf::from("folder/file")
    );
}

#[test]
fn directory_listing_uses_name_paths_and_node_capabilities() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("Folder")).unwrap();
    std::fs::write(root.path().join("notes.txt"), b"notes").unwrap();
    let backend = TrashDirectory::with_root(root.path().to_path_buf());

    let listing = backend.list(TRASH_URI).unwrap();
    let file = listing
        .entries
        .iter()
        .find(|entry| entry.name == "notes.txt")
        .unwrap();
    let folder = listing
        .entries
        .iter()
        .find(|entry| entry.name == "Folder")
        .unwrap();
    assert_eq!(file.node_id, "trash://notes.txt");
    assert!(!file
        .node_id
        .contains(root.path().to_string_lossy().as_ref()));
    assert!(file.allows(FsOperation::Remove));
    assert!(!file.allows(FsOperation::List));
    assert!(folder.allows(FsOperation::List));
    backend.validate_child(TRASH_URI, &file.node_id).unwrap();
    assert_eq!(backend.entry(&file.node_id).unwrap().name, "notes.txt");
    assert_eq!(backend.parent_uri(&file.node_id).unwrap(), TRASH_URI);
}

#[cfg(unix)]
#[test]
fn intermediate_trash_symlinks_cannot_escape_the_root() {
    let root = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink("/tmp", root.path().join("escape")).unwrap();
    assert!(confined(root.path(), Path::new("escape/child")).is_err());
}

#[cfg(target_os = "linux")]
#[test]
fn permanent_delete_removes_the_freedesktop_origin_record() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("Trash/files");
    let info = temp.path().join("Trash/info");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(&info).unwrap();
    std::fs::write(root.join("item.txt"), b"trash").unwrap();
    let record = info.join("item.txt.trashinfo");
    std::fs::write(&record, "[Trash Info]\nPath=/tmp/item.txt\n").unwrap();
    let backend = TrashDirectory::with_root(root);
    let item = backend
        .list(TRASH_URI)
        .unwrap()
        .entries
        .into_iter()
        .next()
        .unwrap();

    backend.purge(&item.node_id).unwrap();

    assert!(!record.exists());
}

#[cfg(target_os = "linux")]
#[test]
fn deleting_nested_trash_items_keeps_top_level_origin_records() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("Trash/files");
    let info = temp.path().join("Trash/info");
    std::fs::create_dir_all(root.join("outer")).unwrap();
    std::fs::create_dir_all(&info).unwrap();
    std::fs::write(root.join("outer/item.txt"), b"nested").unwrap();
    let record = info.join("item.txt.trashinfo");
    std::fs::write(&record, "[Trash Info]\nPath=/tmp/item.txt\n").unwrap();
    let backend = TrashDirectory::with_root(root);
    let parent = backend.list(TRASH_URI).unwrap().entries.remove(0);
    let child = backend.list(&parent.node_id).unwrap().entries.remove(0);
    assert_eq!(parent.node_id, "trash://outer");
    assert_eq!(child.node_id, "trash://outer/item.txt");
    assert_eq!(backend.parent_uri(&child.node_id).unwrap(), parent.node_id);

    backend.purge(&child.node_id).unwrap();

    assert!(record.exists());
}
