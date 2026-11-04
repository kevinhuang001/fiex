use super::*;
use crate::core::volumes::{recycle, trash_vfs};

#[test]
fn only_view_specific_actions_have_native_view_providers() {
    assert_eq!(
        PROVIDER_IDS,
        [
            "builtin:reveal",
            "builtin:remove-from-recent",
            "builtin:restore",
            "builtin:empty-trash"
        ]
    );
    assert!(providers("applications://", &["applications://system-app".into()]).is_empty());
}

#[test]
fn restore_is_single_selection_and_empty_trash_is_root_only() {
    let paths = vec!["trash://one".into(), "trash://two".into()];
    assert_eq!(
        providers(trash_vfs::URI, &paths[..1])[0].id,
        "builtin:restore"
    );
    assert!(providers(trash_vfs::URI, &paths).is_empty());

    for directory in [trash_vfs::URI, recycle::URI] {
        let candidates = providers(directory, &[]);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].id, "builtin:empty-trash");
        let resolved = resolve("builtin:empty-trash", directory, &[]).unwrap();
        assert_eq!(resolved["items"].as_array().unwrap()[0]["id"], "emptyTrash");
        assert_eq!(
            execute("builtin:empty-trash", "emptyTrash", directory, &[]).unwrap()["command"],
            "emptyTrash"
        );
    }
    assert!(
        resolve("builtin:empty-trash", "trash://nested", &[]).unwrap()["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(providers("recent://", &[]).is_empty());
    let recent = vec!["recent://one".into(), "recent://two".into()];
    assert_eq!(
        providers("recent://", &recent)[0].id,
        "builtin:remove-from-recent"
    );
    let resolved = resolve("builtin:remove-from-recent", "recent://", &recent).unwrap();
    assert_eq!(
        resolved["items"].as_array().unwrap()[0]["id"],
        "removeFromRecent"
    );
    assert!(providers("/tmp", &[]).is_empty());
}

#[test]
fn restore_returns_a_view_scoped_action_for_the_exact_selection() {
    let paths = vec!["trash://opaque-node".into()];
    let result = resolve("builtin:restore", trash_vfs::URI, &paths).unwrap();
    assert_eq!(result["items"].as_array().unwrap()[0]["id"], "restore");
    let effect = execute("builtin:restore", "restore", trash_vfs::URI, &paths).unwrap();
    assert_eq!(effect["command"], "restore");
    assert_eq!(effect["paths"], json!(paths));
}
