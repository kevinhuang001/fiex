use super::*;
use crate::remote::sync::{RemoteMeta, RemoteMetaFile};
fn remote(name: &str) -> RemoteFile {
    RemoteFile {
        name: name.into(),
        is_dir: false,
        size: 12,
        modified: 0,
        node_id: "stable-id".into(),
        revision: "new-version".into(),
        allowed_ops: Some(vec!["read".into(), "write".into()]),
    }
}
#[test]
fn offline_listing_survives_metadata_round_trip() {
    let mut meta = RemoteMeta::default();
    remember_listing("folder", &[remote("file.txt")], &mut meta, true);
    let recovered: RemoteMeta =
        serde_json::from_slice(&serde_json::to_vec(&meta).unwrap()).unwrap();
    assert!(recovered.listed.contains("folder"));
    let files = cached_listing("folder", &recovered);
    assert_eq!(files.len(), 1);
    assert_eq!(recovered.listed_children["folder"], ["folder/file.txt"]);
    assert_eq!(files[0].node_id, "stable-id");
    assert_eq!(
        files[0].allowed_ops.as_deref(),
        Some(&["read".to_string(), "write".to_string()][..])
    );
    assert!(cached_listing("other", &recovered).is_empty());
}

#[test]
fn complete_offline_listing_does_not_resurrect_removed_remote_children() {
    let mut meta = RemoteMeta::default();
    remember_listing(
        "folder",
        &[remote("old.txt"), remote("kept.txt")],
        &mut meta,
        true,
    );
    remember_listing("folder", &[remote("kept.txt")], &mut meta, true);
    let files = cached_listing("folder", &meta);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "kept.txt");
    assert!(meta.files.contains_key("folder/old.txt"));
}
#[test]
fn operations_are_per_node_and_provider_declaration() {
    let plugin = "org.example.capability-test";
    crate::plugins::provider_registry::register(
        plugin,
        &serde_json::json!({"vfs":{"discovery":true,"operations":["list","stat","read","write","remove"],"preflightOps":[]}}),
    );
    let operations = entry_capabilities(plugin, None);
    assert!(operations.iter().any(|op| op == "remove"));
    let read_only = vec!["read".to_string(), "stat".to_string()];
    let operations = entry_capabilities(plugin, Some(&read_only));
    assert_eq!(operations, vec!["stat", "read"]);
    // A node cannot expose an operation the plugin never declared.
    let undeclared = vec!["rename".to_string()];
    assert!(entry_capabilities(plugin, Some(&undeclared)).is_empty());
    crate::plugins::provider_registry::forget(plugin);
    assert!(entry_capabilities(plugin, None).is_empty());
}
#[test]
fn listing_cannot_advance_download_revision() {
    let mut meta = RemoteMeta::default();
    meta.files.insert(
        "file.txt".into(),
        RemoteMetaFile {
            downloaded: true,
            revision: "download-version".into(),
            baseline: "local-hash".into(),
            ..Default::default()
        },
    );
    for _ in 0..10 {
        remember_listing("", &[remote("file.txt")], &mut meta, true);
    }
    assert_eq!(meta.files["file.txt"].revision, "download-version");
    assert_eq!(meta.files["file.txt"].baseline, "local-hash");
    assert!(!meta.files["file.txt"].conflict);
}
#[test]
fn folder_pin_inheritance_respects_explicit_eviction() {
    let mut meta = RemoteMeta::default();
    meta.files.insert(
        "folder".into(),
        RemoteMetaFile {
            pinned: true,
            is_dir: true,
            ..Default::default()
        },
    );
    remember_listing("folder", &[remote("a"), remote("b")], &mut meta, true);
    assert!(meta.files["folder/a"].pinned);
    let b = meta.files.get_mut("folder/b").unwrap();
    b.excluded = true;
    b.pinned = false;
    remember_listing("folder", &[remote("b")], &mut meta, true);
    assert!(!meta.files["folder/b"].pinned);
}
