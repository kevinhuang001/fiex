use super::*;
#[test]
fn open_read_keeps_the_selected_file_and_supports_sparse_large_offsets() {
    let root = tempfile::tempdir().unwrap();
    let chosen = root.path().join("chosen");
    let mut file = std::fs::File::create(&chosen).unwrap();
    let far = 5_u64 * 1024 * 1024 * 1024;
    file.seek(SeekFrom::Start(far)).unwrap();
    file.write_all(&[0, 255]).unwrap();
    let scope = Scope::selected(root.path(), std::slice::from_ref(&chosen)).unwrap();
    let value = open_read("owner", &scope, "chosen").unwrap();
    let handle = value["handle"].as_str().unwrap();
    assert_eq!(value["size"], far + 2);
    assert!(http_body("other", handle).is_err());
    let mut upload = http_body("owner", handle).unwrap();
    assert_eq!(upload.metadata().unwrap().len(), far + 2);
    assert_eq!(upload.stream_position().unwrap(), 0);
    assert!(read_open("other", handle, far, 2, None).is_err());
    std::fs::rename(&chosen, root.path().join("moved")).unwrap();
    std::fs::write(&chosen, b"replacement").unwrap();
    let data = read_open("owner", handle, far, 2, None).unwrap();
    assert_eq!(data["base64"], STANDARD.encode([0, 255]));
    let descriptor = crate::plugins::ipc::create("owner", 4096, false).unwrap();
    assert!(read_open("other", handle, far, 2, Some(&descriptor.id)).is_err());
    assert!(read_open("owner", handle, far, 2, Some(&descriptor.id)).is_ok());
    assert_eq!(
        &crate::plugins::ipc::read("owner", &descriptor.id).unwrap()[..2],
        &[0, 255]
    );
    crate::plugins::ipc::release("owner", &descriptor.id).unwrap();
    close_read("owner", handle).unwrap();
    assert!(http_body("owner", handle).is_err());
    assert!(read_open("owner", handle, 0, 1, None).is_err());
}
#[test]
fn operation_selection_does_not_expose_sibling_files() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("chosen"), b"ok").unwrap();
    std::fs::write(root.path().join("private"), b"secret").unwrap();
    let scope = Scope::selected(root.path(), &[root.path().join("chosen")]).unwrap();
    assert!(read_chunk(&scope, "chosen", 0, 8).is_ok());
    assert!(read_chunk(&scope, "private", 0, 8).is_err());
    assert!(list(&scope, ".").is_err());
    assert!(stat(&scope, "private").is_err());
    let empty = Scope::selected(root.path(), &[]).unwrap();
    assert!(read_chunk(&empty, "chosen", 0, 8).is_err());
}
#[test]
fn batch_stat_keeps_order_and_hides_unselected_entries() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("chosen"), b"ok").unwrap();
    std::fs::write(root.path().join("private"), b"secret").unwrap();
    let scope = Scope::selected(root.path(), &[root.path().join("chosen")]).unwrap();
    let result = stat_batch(
        &scope,
        &[
            json!("chosen"),
            json!("private"),
            json!("chosen"),
            json!(null),
        ],
    )
    .unwrap();
    assert_eq!(result["results"][0]["metadata"]["name"], "chosen");
    assert!(result["results"][1].get("metadata").is_none());
    assert_eq!(result["results"][1]["error"], "permission denied");
    assert_eq!(result["results"][2]["error"], "duplicate selection");
    assert_eq!(result["results"][3]["error"], "invalid path");
    assert!(stat_batch(&scope, &vec![json!("chosen"); 257]).is_err());
}
#[test]
fn an_action_folder_grant_allows_results_next_to_the_selection() {
    let root = tempfile::tempdir().unwrap();
    let selected = root.path().join("chosen.zip");
    std::fs::write(&selected, b"content").unwrap();
    // What an explicit open/context action adds for a plugin that declares
    // create capability: the selected file plus its containing folder.
    let scope =
        Scope::selected(root.path(), &[selected.clone(), root.path().to_path_buf()]).unwrap();
    create(&scope, "chosen-output.zip").unwrap();
    mkdir(&scope, ".staging").unwrap();
    mkdir(&scope, ".staging/nested").unwrap();
    let handle = begin_write("owner", &scope, ".staging/nested/file", true).unwrap();
    write_chunk("owner", &handle, &STANDARD.encode(b"data")).unwrap();
    commit("owner", &handle).unwrap();
    rename(&scope, ".staging/nested/file", "moved.txt").unwrap();
    assert!(read_chunk(&scope, "chosen.zip", 0, 100).is_ok());
    assert_eq!(
        std::fs::read(root.path().join("moved.txt")).unwrap(),
        b"data"
    );
    // The grant is not a blanket escape from the operation root.
    let outside = tempfile::tempdir().unwrap();
    let outside_file = outside.path().join("secret");
    std::fs::write(&outside_file, b"secret").unwrap();
    assert!(read_chunk(&scope, outside_file.to_str().unwrap(), 0, 100).is_err());
}

#[test]
fn selected_directory_allows_descendants_only() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("chosen")).unwrap();
    std::fs::write(root.path().join("chosen/file"), b"ok").unwrap();
    std::fs::write(root.path().join("other"), b"secret").unwrap();
    let scope = Scope::selected(root.path(), &[root.path().join("chosen")]).unwrap();
    assert!(read_chunk(&scope, "chosen/file", 0, 8).is_ok());
    assert!(read_chunk(&scope, "other", 0, 8).is_err());
}
#[test]
fn save_selection_allows_only_the_new_target() {
    let root = tempfile::tempdir().unwrap();
    let scope = Scope::selected(root.path(), &[root.path().join("output.txt")]).unwrap();
    let result = create(&scope, "output.txt");
    assert!(result.is_ok(), "{result:?}");
    assert!(create(&scope, "other.txt").is_err());
}
#[cfg(unix)]
#[test]
fn selected_folder_cannot_follow_a_link_to_its_sibling() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("chosen")).unwrap();
    std::fs::write(root.path().join("private"), b"secret").unwrap();
    std::os::unix::fs::symlink("../private", root.path().join("chosen/link")).unwrap();
    let scope = Scope::selected(root.path(), &[root.path().join("chosen")]).unwrap();
    assert!(read_chunk(&scope, "chosen/link", 0, 8).is_err());
}
#[test]
fn staged_writes_preserve_conflicts_and_abort() {
    let root = tempfile::tempdir().unwrap();
    let scope = Scope::new(root.path()).unwrap();
    std::fs::write(root.path().join("file"), b"old").unwrap();
    let handle = begin_write("p", &scope, "file", false).unwrap();
    write_chunk("p", &handle, &STANDARD.encode(b"new")).unwrap();
    assert_eq!(std::fs::read(root.path().join("file")).unwrap(), b"old");
    assert!(write_chunk("other", &handle, "").is_err());
    std::fs::write(root.path().join("file"), b"external").unwrap();
    assert!(commit("p", &handle).is_err());
    abort("p", &handle).unwrap();
    assert_eq!(
        std::fs::read(root.path().join("file")).unwrap(),
        b"external"
    );
    let handle = begin_write("p", &scope, "created", true).unwrap();
    write_chunk("p", &handle, &STANDARD.encode([0, 255, 128])).unwrap();
    commit("p", &handle).unwrap();
    assert_eq!(
        std::fs::read(root.path().join("created")).unwrap(),
        [0, 255, 128]
    );
    assert!(remove(&scope, ".").is_err());
}
#[test]
fn shared_write_is_owned_bounded_and_transactional() {
    let root = tempfile::tempdir().unwrap();
    let scope = Scope::selected(root.path(), &[root.path().join("new")]).unwrap();
    let handle = begin_write("owner", &scope, "new", true).unwrap();
    let buffer = crate::plugins::ipc::create("owner", 256 * 1024, true).unwrap();
    crate::plugins::ipc::write_owned("owner", &buffer.id, &[0, 255, 42]).unwrap();
    assert!(write_buffer("other", &handle, &buffer.id, 3).is_err());
    assert!(write_buffer("owner", &handle, &buffer.id, MAX_CHUNK + 1).is_err());
    write_buffer("owner", &handle, &buffer.id, 3).unwrap();
    assert!(!root.path().join("new").exists());
    commit("owner", &handle).unwrap();
    assert_eq!(
        std::fs::read(root.path().join("new")).unwrap(),
        [0, 255, 42]
    );
    crate::plugins::ipc::release("owner", &buffer.id).unwrap();
}
#[cfg(unix)]
#[test]
fn symlinks_cannot_escape_task_root() {
    let root = tempfile::tempdir().unwrap();
    let scope = Scope::new(root.path()).unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret"), "secret").unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("link")).unwrap();
    assert!(read_chunk(&scope, "link/secret", 0, 100).is_err());
    assert!(begin_write("p", &scope, "link/secret", false).is_err());
}
#[cfg(unix)]
#[test]
fn replacing_the_file_identity_aborts_commit() {
    let root = tempfile::tempdir().unwrap();
    let scope = Scope::new(root.path()).unwrap();
    let path = root.path().join("file");
    std::fs::write(&path, b"old").unwrap();
    let handle = begin_write("owner", &scope, "file", false).unwrap();
    write_chunk("owner", &handle, &STANDARD.encode(b"new")).unwrap();
    std::fs::remove_file(&path).unwrap();
    std::fs::write(&path, b"other").unwrap();
    assert!(commit("owner", &handle).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"other");
}
#[test]
fn a_wrong_owner_cannot_consume_another_operation_transaction() {
    let root = tempfile::tempdir().unwrap();
    let scope = Scope::new(root.path()).unwrap();
    std::fs::write(root.path().join("file"), b"old").unwrap();
    let handle = begin_write("owner", &scope, "file", false).unwrap();
    write_chunk("owner", &handle, &STANDARD.encode(b"new")).unwrap();
    assert!(commit("other", &handle).is_err());
    assert_eq!(std::fs::read(root.path().join("file")).unwrap(), b"old");
    commit("owner", &handle).unwrap();
    assert_eq!(std::fs::read(root.path().join("file")).unwrap(), b"new");
}
#[cfg(unix)]
#[test]
fn captured_directory_survives_root_path_replacement() {
    let parent = tempfile::tempdir().unwrap();
    let original = parent.path().join("original");
    let moved = parent.path().join("moved");
    let outside = parent.path().join("outside");
    std::fs::create_dir(&original).unwrap();
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(original.join("file"), b"bound").unwrap();
    std::fs::write(outside.join("file"), b"outside").unwrap();
    let scope = Scope::new(&original).unwrap();
    let handle = begin_write("captured", &scope, "file", false).unwrap();
    write_chunk("captured", &handle, &STANDARD.encode(b"updated")).unwrap();
    std::fs::rename(&original, &moved).unwrap();
    std::os::unix::fs::symlink(&outside, &original).unwrap();
    let read = read_chunk(&scope, "file", 0, 100).unwrap();
    assert_eq!(read["base64"], STANDARD.encode(b"bound"));
    commit("captured", &handle).unwrap();
    assert_eq!(std::fs::read(moved.join("file")).unwrap(), b"updated");
    assert_eq!(std::fs::read(outside.join("file")).unwrap(), b"outside");
}

fn create(root: &Scope, input: &str) -> Result<Value, String> {
    let dir = directory(root)?;
    let path = relative(root, input)?;
    dir.open_with(
        &path,
        OpenOptions::new().read(true).write(true).create_new(true),
    )
    .map_err(|e| e.to_string())?;
    entry(&dir, &path)
}
