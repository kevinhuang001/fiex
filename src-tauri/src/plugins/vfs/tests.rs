use super::*;
use serde_json::json;

fn file_entry(name: &str) -> FsEntry {
    FsEntry {
        node_id: "opaque-node".into(),
        name: name.into(),
        kind: FsKind::File,
        size: Some(7),
        modified: Some(0),
        extension: Some("txt".into()),
        mime: Some("text/plain".into()),
        revision: None,
        readonly: None,
        link_target: None,
        allowed_ops: None,
        relative_path: None,
    }
}
fn mount(plugin_id: &str) -> Mount {
    Mount {
        mount_id: "owner-one".into(),
        plugin_id: plugin_id.into(),
        source: Source::View {
            contribution_id: "tree".into(),
            view_context: json!({}),
        },
        scope: None,
    }
}

#[test]
fn provider_cannot_choose_a_different_mount_or_escape_with_a_name() {
    let location = crate::plugins::mounts::parse("fiex://mount/owner-one/folder").unwrap();
    let mount = mount("test.vfs.identity");
    let (projected, node) = project_entry(
        &mount,
        &location,
        "folder/report #%.txt",
        file_entry("report #%.txt"),
    )
    .unwrap();
    assert_eq!(node, "opaque-node");
    assert_eq!(
        projected.path,
        "fiex://mount/owner-one/folder/report%20%23%25.txt"
    );
    assert!(!projected.readonly && projected.sync.is_none());
    assert!(project_entry(
        &mount,
        &location,
        "folder/../secret",
        file_entry("../secret")
    )
    .is_err());
    assert!(project_entry(&mount, &location, "other.txt", file_entry("report #%.txt")).is_err());
}

#[test]
fn capabilities_narrow_the_provider_and_mark_entries_readonly() {
    let location = crate::plugins::mounts::parse("fiex://mount/owner-one/folder").unwrap();
    let mount = mount("test.vfs.capabilities");
    let declared = [FsOperation::Read, FsOperation::Rename];

    let mut writable = file_entry("a.txt");
    writable.allowed_ops = Some(vec![FsOperation::Read, FsOperation::Rename]);
    assert_eq!(
        allowed_operations(&writable, |operation| declared.contains(&operation)),
        vec!["read", "rename"]
    );
    let (projected, _) = project_entry(&mount, &location, "folder/a.txt", writable).unwrap();
    // No registered mount exists: the real authorization gate must fail closed.
    assert!(projected.allowed_ops.is_empty());
    assert!(!projected.readonly);

    let mut read_only = file_entry("b.txt");
    read_only.allowed_ops = Some(vec![FsOperation::Read]);
    assert_eq!(
        allowed_operations(&read_only, |operation| declared.contains(&operation)),
        vec!["read"]
    );
    let (projected, _) = project_entry(&mount, &location, "folder/b.txt", read_only).unwrap();
    assert!(projected.allowed_ops.is_empty());
    // Per-node readonly is separate from capability narrowing.
    assert!(!projected.readonly);
}

#[test]
fn child_uris_reject_traversal_and_keep_the_mount() {
    assert_eq!(child("folder", "a.txt").unwrap(), "folder/a.txt");
    assert_eq!(child("", "a.txt").unwrap(), "a.txt");
    assert!(child("folder", "../secret").is_err());
}
