use super::*;

#[test]
fn desktop_listing_and_info_share_broker_and_preserve_projection() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("visible.txt"), b"data").unwrap();
    std::fs::write(dir.path().join(".hidden"), b"hidden").unwrap();
    let broker = trusted(&dir.path().to_string_lossy(), "core:test-list").unwrap();
    let listing = broker.desktop_listing(false, &[]).unwrap();
    assert_eq!(listing.entries.len(), 1);
    assert_eq!(listing.entries[0].name, "visible.txt");
    assert!(!listing.virtual_volume);
    let item = trusted(&listing.entries[0].path, "core:test-stat")
        .unwrap()
        .desktop_entry()
        .unwrap();
    assert_eq!(item.size, 4);
    assert_eq!(item.mime, "text/plain");
    assert_eq!(broker.desktop_listing(true, &[]).unwrap().entries.len(), 2);
}
#[test]
fn filesystem_root_can_be_captured_as_directory() {
    let dir = std::env::temp_dir();
    let root = dir.ancestors().last().unwrap();
    let resource = trusted(&root.to_string_lossy(), "core:test-root").unwrap();
    assert!(resource.desktop_entry().unwrap().is_dir);
}
#[test]
fn provider_symlink_projection_is_not_lost() {
    let item = super::presentation::entry(json!({"name":"link", "symlink":true})).unwrap();
    assert!(item.is_symlink);
}
#[test]
fn feature_discovery_receives_independent_operation_capabilities() {
    use fiex_resource::FileCapabilities;
    let mut resource = crate::plugins::resource::resource_context::from_metadata(
        "example",
        crate::plugins::contracts::ResourceMetadata {
            name: "example.txt".into(),
            kind: "file".into(),
            ..Default::default()
        },
    );
    resource.operations = Some(FileCapabilities {
        stat: true,
        ..Default::default()
    });
    let rules: crate::plugins::match_rules::MatchRules =
        serde_json::from_value(json!({"requiredOps":["read"]})).unwrap();
    assert!(!rules.matches_resources(
        &crate::plugins::events::matching::match_resources(&[resource.clone()]),
        ""
    ));
    resource.operations.as_mut().unwrap().read = true;
    assert!(rules.matches_resources(
        &crate::plugins::events::matching::match_resources(&[resource]),
        ""
    ));
}
#[cfg(unix)]
#[test]
fn symlink_kind_survives_event_metadata_projection() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("file");
    let link = dir.path().join("link");
    std::fs::write(&file, b"bytes").unwrap();
    std::os::unix::fs::symlink(&file, &link).unwrap();
    let resource =
        crate::plugins::resource::resource_context::resolve(&link.to_string_lossy()).unwrap();
    assert_eq!(
        crate::plugins::events::resource_metadata(&[resource])[0]["kind"],
        "symlink"
    );
}
#[test]
fn desktop_search_runs_through_the_same_provider_and_honors_cancellation() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("nested")).unwrap();
    std::fs::write(dir.path().join("nested").join("needle.txt"), b"x").unwrap();
    let root = super::binding::trusted(&dir.path().to_string_lossy(), "core:test-search").unwrap();
    let (entries, truncated, skipped) = root.search("needle", false, &[], 50, &|| false).unwrap();
    assert!(!truncated);
    assert_eq!(skipped, 0);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "needle.txt");
    let (entries, _, _) = root.search("needle", false, &[], 50, &|| true).unwrap();
    assert!(entries.is_empty());
}
#[test]
fn vfs_search_reads_the_cached_view() {
    let root = super::binding::trusted(
        crate::core::volumes::native_vfs::RECENT_URI,
        "core:test-virtual-search",
    )
    .unwrap();
    let (entries, truncated, skipped) = root
        .search("__fiex_virtual_cache_probe__", false, &[], 10, &|| false)
        .unwrap();
    assert!(entries.is_empty());
    assert!(!truncated);
    assert_eq!(skipped, 0);
}
#[test]
fn builtin_view_storage_and_presentation_are_separate() {
    use crate::core::volumes::{applications, native_vfs};
    let root = native_vfs::resource_entry(applications::URI).unwrap();
    assert!(root.is_dir);
    assert_eq!(root.name, "core:applications");
    assert!(root.allowed_ops.iter().any(|op| op == "mkdir"));
    assert_eq!(
        native_vfs::provider_id(applications::URI),
        Some("core:applications")
    );
    assert!(native_vfs::list(native_vfs::RECENT_URI, "core:test-recent").is_ok());
}
