use crate::core::fs::{CHUNK, transfer};

fn assert_paths_resolve_to_same_location(actual: &str, expected: &std::path::Path) {
    assert_eq!(
        std::fs::canonicalize(actual).unwrap(),
        std::fs::canonicalize(expected).unwrap()
    );
}

#[test]
fn nested_directory_transfer_creates_children_via_scoped_refs() {
    let source = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let nested = source.path().join("album").join("nested");
    std::fs::create_dir_all(&nested).unwrap();
    let payload = vec![17u8; CHUNK + 29];
    std::fs::write(nested.join("large.bin"), &payload).unwrap();
    let source_path = source.path().join("album").to_string_lossy().into_owned();
    let target_path = target.path().to_string_lossy().into_owned();
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let result = transfer(
        &[source_path],
        &target_path,
        false,
        false,
        &[],
        &[],
        &cancel,
        &|_, _, _| {},
    )
    .unwrap();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(
        std::fs::read(target.path().join("album/nested/large.bin")).unwrap(),
        payload
    );
}

#[test]
fn keep_both_copy_in_source_parent_creates_a_suffixed_sibling() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("report.txt");
    std::fs::write(&source, b"report contents").unwrap();
    let source_path = source.to_string_lossy().into_owned();
    let destination = directory.path().to_string_lossy().into_owned();
    let cancel = std::sync::atomic::AtomicBool::new(false);

    let result = transfer(
        &[source_path],
        &destination,
        false,
        true,
        &[],
        &[],
        &cancel,
        &|_, _, _| {},
    )
    .unwrap();

    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(result.paths.len(), 1);
    assert_paths_resolve_to_same_location(
        &result.paths[0],
        &directory.path().join("report (1).txt"),
    );
    assert_eq!(
        std::fs::read(directory.path().join("report.txt")).unwrap(),
        b"report contents"
    );
    assert_eq!(
        std::fs::read(directory.path().join("report (1).txt")).unwrap(),
        b"report contents"
    );
}

#[test]
fn duplicate_directory_in_source_parent_creates_a_suffixed_sibling() {
    let parent = tempfile::tempdir().unwrap();
    let source = parent.path().join("assets");
    std::fs::create_dir_all(source.join("nested")).unwrap();
    std::fs::write(source.join("nested/image.png"), b"image bytes").unwrap();
    let source_path = source.to_string_lossy().into_owned();
    let destination = parent.path().to_string_lossy().into_owned();
    let cancel = std::sync::atomic::AtomicBool::new(false);

    let result = transfer(
        &[source_path],
        &destination,
        false,
        true,
        &[],
        &[],
        &cancel,
        &|_, _, _| {},
    )
    .unwrap();

    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(result.paths.len(), 1);
    assert_paths_resolve_to_same_location(&result.paths[0], &parent.path().join("assets (1)"));
    assert!(source.join("nested/image.png").is_file());
    assert_eq!(
        std::fs::read(parent.path().join("assets (1)/nested/image.png")).unwrap(),
        b"image bytes"
    );
}

#[test]
fn cancelled_multi_file_copy_rolls_back_committed_destinations() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let source = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let a = source.path().join("a.txt");
    let b = source.path().join("b.txt");
    std::fs::write(&a, b"a").unwrap();
    std::fs::write(&b, b"b").unwrap();
    let files = [
        a.to_string_lossy().into_owned(),
        b.to_string_lossy().into_owned(),
    ];
    let destination = target.path().to_string_lossy().into_owned();
    let cancel = AtomicBool::new(false);
    let result = transfer(
        &files,
        &destination,
        true,
        false,
        &[],
        &[],
        &cancel,
        &|done, _, _| {
            if done == 1 {
                cancel.store(true, Ordering::SeqCst);
            }
        },
    )
    .unwrap();
    assert!(!result.errors.is_empty());
    assert!(!target.path().join("a.txt").exists());
    assert!(!target.path().join("b.txt").exists());
    assert_eq!(std::fs::read(a).unwrap(), b"a");
    assert_eq!(std::fs::read(b).unwrap(), b"b");
}

#[test]
fn failed_multi_file_copy_rolls_back_previously_committed_destinations() {
    let source = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let first = source.path().join("first.txt");
    let second = source.path().join("second.txt");
    std::fs::write(&first, b"first").unwrap();
    std::fs::write(&second, b"before").unwrap();
    let files = [
        first.to_string_lossy().into_owned(),
        second.to_string_lossy().into_owned(),
    ];
    let destination = target.path().to_string_lossy().into_owned();
    let cancel = std::sync::atomic::AtomicBool::new(false);

    let result = transfer(
        &files,
        &destination,
        false,
        false,
        &[],
        &[],
        &cancel,
        &|done, _, _| {
            if done == 1 {
                std::fs::write(&second, b"changed after planning").unwrap();
            }
        },
    )
    .unwrap();

    assert!(result.paths.is_empty());
    assert_eq!(result.errors.len(), 1, "{:?}", result.errors);
    assert!(!target.path().join("first.txt").exists());
    assert!(!target.path().join("second.txt").exists());
    assert_eq!(std::fs::read(first).unwrap(), b"first");
    assert_eq!(std::fs::read(second).unwrap(), b"changed after planning");
}

#[test]
fn provider_write_failure_rolls_back_previously_committed_destinations() {
    use crate::core::operation_types::{Action, Resolution};
    use std::sync::atomic::AtomicBool;

    let source = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let first = source.path().join("first.txt");
    let source_folder = source.path().join("bundle");
    let nested_source = source_folder.join("nested.txt");
    let target_folder = target.path().join("bundle");
    std::fs::write(&first, b"first").unwrap();
    std::fs::create_dir(&source_folder).unwrap();
    std::fs::write(&nested_source, b"nested").unwrap();
    std::fs::create_dir(&target_folder).unwrap();
    let nested_target = target_folder.join("nested.txt");
    std::fs::write(&nested_target, b"original target").unwrap();

    let (source_scope, source_ref) = super::binding(source.path(), &source_folder);
    let source_broker = crate::core::fs::ResourceBroker::resolve(
        &source_scope,
        "core:provider-write-failure",
        &source_ref,
    )
    .unwrap();
    let (target_scope, target_ref) = super::binding(target.path(), &target_folder);
    let target_broker = crate::core::fs::ResourceBroker::resolve(
        &target_scope,
        "core:provider-write-failure",
        &target_ref,
    )
    .unwrap();
    let nested_source_broker = super::binding::child(
        &source_broker,
        "nested.txt",
        crate::core::fs::provider_registry::ChildKind::File,
    )
    .unwrap();
    let nested_target_broker = super::binding::child(
        &target_broker,
        "nested.txt",
        crate::core::fs::provider_registry::ChildKind::File,
    )
    .unwrap();
    let source_version = source_broker.stat().unwrap();
    let target_version = target_broker.stat().unwrap();
    let key = format!(
        "{}\0{}\0{}\0{}\0{}\0{}",
        source_folder.display(),
        target_folder.display(),
        source_version["modified"].as_u64().unwrap_or(0),
        target_version["modified"].as_u64().unwrap_or(0),
        source_version["size"].as_u64().unwrap_or(0),
        target_version["size"].as_u64().unwrap_or(0),
    );
    let nested_source_version = nested_source_broker.stat().unwrap();
    let nested_target_version = nested_target_broker.stat().unwrap();
    let nested_key = format!(
        "{}\0{}\0{}\0{}\0{}\0{}",
        nested_source_broker.path,
        nested_target_broker.path,
        nested_source_version["modified"].as_u64().unwrap_or(0),
        nested_target_version["modified"].as_u64().unwrap_or(0),
        nested_source_version["size"].as_u64().unwrap_or(0),
        nested_target_version["size"].as_u64().unwrap_or(0),
    );
    let paths = [
        first.to_string_lossy().into_owned(),
        source_folder.to_string_lossy().into_owned(),
    ];
    let destination = target.path().to_string_lossy().into_owned();
    crate::core::fs::provider_registry::fail_next_provider_write_for_test(
        &target_folder.join("nested.txt").to_string_lossy(),
    );
    let result = transfer(
        &paths,
        &destination,
        false,
        false,
        &[],
        &[
            Resolution {
                key,
                action: Action::Replace,
            },
            Resolution {
                key: nested_key,
                action: Action::Replace,
            },
        ],
        &AtomicBool::new(false),
        &|_, _, _| {},
    )
    .unwrap();

    assert!(result.paths.is_empty());
    assert_eq!(result.errors.len(), 1, "{:?}", result.errors);
    assert!(
        result.errors[0].contains("injected provider write failure"),
        "{:?}",
        result.errors
    );
    assert!(!target.path().join("first.txt").exists());
    assert_eq!(std::fs::read(&nested_target).unwrap(), b"original target");
    assert_eq!(std::fs::read(&first).unwrap(), b"first");
    assert_eq!(std::fs::read(nested_source).unwrap(), b"nested");
}
