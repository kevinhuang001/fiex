use super::types::DecorationTag;
use super::*;

fn entry_at(path: &str, sync: &str) -> DecorationEntry {
    DecorationEntry {
        path: path.into(),
        name: "a".into(),
        sync: Some(sync.into()),
        ..Default::default()
    }
}
fn entry(sync: &str) -> DecorationEntry {
    entry_at("/no-io/a", sync)
}
#[test]
fn builtin_decorations_use_eventrouter_without_plugins_or_file_reads() {
    let mut value = entry_at("applications://test", "pinned");
    value.pinned = true;
    value.uploaded = true;
    let result = query_with_plugins("applications://", vec![value], false);
    assert_eq!(result.len(), 1);
    let badges = &result[0].native_badges;
    assert_eq!(badges.len(), 2);
    assert_eq!(badges[0].status.as_deref(), Some("pinned"));
    assert_eq!(badges[1].status.as_deref(), Some("uploaded"));
    assert_eq!(result[0].providers, vec!["Fiex"]);
}
#[test]
fn uploaded_requires_cached_or_pinned_copy_and_unknown_status_is_ignored() {
    let mut value = entry_at("applications://cloud", "cloud");
    value.uploaded = true;
    let result = query_with_plugins("applications://", vec![value], false);
    assert_eq!(result[0].native_badges.len(), 1);
    assert!(query_with_plugins(
        "applications://",
        vec![entry_at("applications://unknown", "invented")],
        false
    )
    .is_empty());
}
#[test]
fn tags_readonly_and_conflict_survive_collection_without_badge_truncation() {
    let mut value = entry("");
    value.readonly = true;
    value.tags = (0..7)
        .map(|index| DecorationTag {
            name: format!("tag{index}"),
            color: 4,
            hex: None,
        })
        .collect();
    let result = query_with_plugins("/no-io", vec![value], false);
    let badges = &result[0].native_badges;
    assert_eq!(badges.len(), 6);
    assert!(badges[..5].iter().all(|badge| badge.dot));
    assert_eq!(badges[5].text, crate::i18n::t("只读"));

    let conflict = entry_at("applications://conflict", "conflict");
    let result = query_with_plugins("applications://", vec![conflict], false);
    assert_eq!(result[0].native_badges.len(), 1);
    assert_eq!(
        result[0].native_badges[0].status.as_deref(),
        Some("conflict")
    );
}
