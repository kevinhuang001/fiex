use super::*;

fn entry(path: &str, allowed: &[&str], readonly: bool) -> Entry {
    Entry {
        path: path.into(),
        allowed_ops: allowed.iter().map(|value| value.to_string()).collect(),
        readonly,
        ..Entry::default()
    }
}

#[test]
fn absent_paths_fail_closed() {
    assert_eq!(
        for_path("/fiex-absent-capability-test/example"),
        FileCapabilities::default()
    );
    let local = entry("/tmp/example", &[], true);
    assert_eq!(
        for_selection(&["/tmp/example".into()], &[local]),
        FileCapabilities {
            stat: true,
            read: true,
            rename: true,
            remove: !cfg!(windows),
            ..FileCapabilities::default()
        }
    );
}

#[test]
fn local_node_capabilities_respect_file_vs_directory_kind() {
    let file = entry("/tmp/document.txt", &[], false);
    let file_caps = for_entry(&file.path, &file);
    assert!(file_caps.read && file_caps.write && file_caps.rename);
    assert!(!file_caps.list && !file_caps.search && !file_caps.create);

    let mut folder = entry("/tmp/folder", &[], false);
    folder.is_dir = true;
    let folder_caps = for_entry(&folder.path, &folder);
    assert!(folder_caps.list && folder_caps.search && folder_caps.create);
    assert!(!folder_caps.read && !folder_caps.write);
}

#[test]
fn native_virtual_search_is_advertised_only_for_listable_directories() {
    let recent = for_path("recent://");
    assert!(
        recent.list && recent.search && recent.stat,
        "Recent root capabilities were {recent:?}"
    );
    assert!(!recent.read && !recent.remove && !recent.write);
    let applications = for_path("applications://");
    assert!(applications.list && applications.search && applications.create);
    assert!(!applications.remove);

    let mut folder = entry(
        "trash://Zm9sZGVy",
        &["list", "stat", "search", "remove"],
        false,
    );
    folder.is_dir = true;
    let capabilities = from_entry(
        &[FsOperation::List, FsOperation::Stat, FsOperation::Search],
        &folder,
    );
    assert!(capabilities.list && capabilities.search && capabilities.stat);
    assert!(!capabilities.remove && !capabilities.write);
    let file = entry("trash://ZmlsZQ", &["stat", "search", "remove"], false);
    let capabilities = from_entry(&[FsOperation::Stat, FsOperation::Search], &file);
    assert!(capabilities.stat && !capabilities.search && !capabilities.remove);
}

#[test]
fn unsupported_uri_never_inherits_local_capabilities() {
    for path in ["unknown://forged.txt", "unknown:opaque-file"] {
        let forged = entry(path, &["read", "write", "remove"], false);
        assert_eq!(for_entry(path, &forged), FileCapabilities::default());
        assert_eq!(for_path(path), FileCapabilities::default());
    }
}

#[test]
fn unknown_mounts_offer_no_operation() {
    let path = "fiex://mount/test-capability-missing/file.txt";
    assert_eq!(for_path(path), FileCapabilities::default());
    let error = require(path, FsOperation::Remove).unwrap_err();
    assert!(error.contains("remove"), "{error}");
}

#[test]
fn declared_operations_are_narrowed_by_the_node() {
    let declared = [FsOperation::Read, FsOperation::Rename, FsOperation::Remove];
    let narrowed = from_entry(&declared, &entry("/m/a", &["read"], false));
    assert!(narrowed.read);
    assert!(!narrowed.stat && !narrowed.list && !narrowed.search);
    let removable = from_entry(&declared, &entry("/m/a", &["read", "remove"], false));
    assert!(removable.read && removable.remove);
    assert!(!removable.rename && !removable.create);
    // Empty explicitly means no operations, even when the provider is broad.
    let empty = from_entry(&declared, &entry("/m/a", &[], false));
    assert_eq!(empty, FileCapabilities::default());
    // READONLY does not replace per-operation parent/server permissions.
    let readonly = from_entry(&declared, &entry("/m/a", &["remove"], true));
    assert!(readonly.remove && !readonly.read);
}

#[test]
fn trash_does_not_grant_generic_delete() {
    let trashed = entry("trash://an-item", &["stat", "remove"], false);
    let capabilities = from_entry(&[FsOperation::Stat], &trashed);
    assert!(capabilities.stat);
    assert!(!capabilities.list && !capabilities.read && !capabilities.remove);
    // Nonexistent opaque nodes fail closed, even though their provider is
    // registered and supports stat on actual items.
    assert!(!for_path("trash://an-item").remove);
    assert!(!for_path("trash://an-item").stat);
    assert!(for_path("trash://").list);
    let mut nested = entry("trash://opaque-folder", &["stat", "list", "remove"], false);
    nested.is_dir = true;
    let capabilities = from_entry(&[FsOperation::Stat, FsOperation::List], &nested);
    assert!(capabilities.stat && capabilities.list);
    assert!(!capabilities.remove && !capabilities.write);
}

#[test]
fn selections_intersect() {
    let empty = FileCapabilities::default();
    assert_eq!(FileCapabilities::ALL.intersect(empty), empty);
    assert_eq!(
        FileCapabilities::ALL.intersect(FileCapabilities::READ_ONLY),
        FileCapabilities::READ_ONLY
    );
    let first = FileCapabilities {
        remove: true,
        ..FileCapabilities::READ_ONLY
    };
    assert!(first
        .intersect(FileCapabilities::READ_ONLY)
        .allows(FsOperation::Read));
    assert!(!first
        .intersect(FileCapabilities::READ_ONLY)
        .allows(FsOperation::Remove));
    // Local paths keep the selection unrestricted when entries are empty.
    assert_eq!(
        for_selection(&["/fiex-absent-capability-test/a".into()], &[]),
        FileCapabilities::default()
    );
    let error = require_selection(
        &["fiex://mount/test-capability-missing/a".into()],
        &[],
        FsOperation::Mkdir,
    )
    .unwrap_err();
    assert!(error.contains("mkdir"), "{error}");
}
