use super::*;
use std::path::Path;

#[test]
fn trash_uris_reject_absolute_and_traversal_paths() {
    for value in ["../outside", "/etc/passwd", "a/../../outside"] {
        let uri = format!("{TRASH_URI}{}", URL_SAFE_NO_PAD.encode(value));
        assert!(relative(&uri).is_err());
    }
    assert_eq!(
        relative(&format!(
            "{TRASH_URI}{}",
            URL_SAFE_NO_PAD.encode("folder/file")
        ))
        .unwrap(),
        PathBuf::from("folder/file")
    );
}

#[test]
fn directory_listing_uses_opaque_ids_and_node_capabilities() {
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
    assert!(file.node_id.starts_with(TRASH_URI));
    assert!(!file
        .node_id
        .contains(root.path().to_string_lossy().as_ref()));
    assert!(file.allows(FsOperation::Remove));
    assert!(!file.allows(FsOperation::List));
    assert!(folder.allows(FsOperation::List));
    backend.validate_child(TRASH_URI, &file.node_id).unwrap();
    assert_eq!(backend.entry(&file.node_id).unwrap().name, "notes.txt");
}

#[cfg(unix)]
#[test]
fn intermediate_trash_symlinks_cannot_escape_the_root() {
    let root = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink("/tmp", root.path().join("escape")).unwrap();
    assert!(confined(root.path(), Path::new("escape/child")).is_err());
}
