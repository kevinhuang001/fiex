//! Declarative definitions for the atomic built-in file-operation actions.
//!
//! Core attaches executable implementations and trusted operation context;
//! this crate owns action identity, ordering, and static provider match rules.

use fiex_provider_contracts::{
    FsOperation, LocationMatch, ProviderMatchExclude, ProviderMatchRule,
};

pub const MODULE_ID: &str = "core:actions";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Open,
    OpenWith,
    Preview,
    Info,
    Tags,
    NewFolder,
    NewFile,
    Rename,
    BatchRename,
    Copy,
    Cut,
    Duplicate,
    CopyPath,
    MoveToTrash,
    DeletePermanently,
    Pin,
    Unpin,
    Cache,
    Uncache,
    ConflictResolution,
}

pub struct ActionDefinition {
    pub id: &'static str,
    pub priority: i64,
    pub rule: ProviderMatchRule,
    pub action: Action,
}

fn exclude_views(provider_ids: &[&str]) -> ProviderMatchExclude {
    ProviderMatchExclude {
        view: Some(LocationMatch {
            provider_ids: Some(provider_ids.iter().map(|id| (*id).into()).collect()),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn selection(value: &str) -> ProviderMatchRule {
    ProviderMatchRule {
        selection: Some(value.into()),
        ..Default::default()
    }
}

fn required(selection: &str, operations: &[FsOperation]) -> ProviderMatchRule {
    ProviderMatchRule {
        selection: Some(selection.into()),
        required_ops: operations.to_vec(),
        ..Default::default()
    }
}

fn storage_vfs(selection: &str) -> ProviderMatchRule {
    ProviderMatchRule {
        selection: Some(selection.into()),
        storage: Some(LocationMatch {
            types: Some(vec!["vfs".into()]),
            ..Default::default()
        }),
        exclude: Some(exclude_views(&["core:trash"])),
        ..Default::default()
    }
}

fn excluding_trash(mut rule: ProviderMatchRule) -> ProviderMatchRule {
    rule.exclude = Some(exclude_views(&["core:trash"]));
    rule
}

/// Static match and priority declarations for the atomic native action menu.
pub fn definitions() -> Vec<ActionDefinition> {
    let mut new_folder = required("zero", &[FsOperation::Mkdir]);
    new_folder.exclude = Some(exclude_views(&["core:trash", "core:recent"]));
    let mut new_file = required("zero", &[FsOperation::Write]);
    new_file.exclude = Some(exclude_views(&[
        "core:applications",
        "core:trash",
        "core:recent",
    ]));
    let mut move_to_trash = required("nonzero", &[FsOperation::Trash]);
    move_to_trash.exclude = Some(exclude_views(&["core:recent", "core:trash"]));
    let mut delete_permanently = required("nonzero", &[FsOperation::Remove]);
    delete_permanently.exclude = Some(exclude_views(&["core:recent"]));
    let mut info = selection("single");
    info.exclude = Some(exclude_views(&["core:trash", "core:applications"]));
    let mut copy_path = selection("nonzero");
    copy_path.exclude = Some(exclude_views(&["core:trash", "core:applications"]));
    let mut open_with = required("single", &[FsOperation::Read]);
    open_with.kinds = vec!["file".into()];
    open_with.exclude = Some(exclude_views(&["core:trash", "core:applications"]));
    // Copy and transfer eligibility depends on resource kind: a file needs
    // Read, while a directory needs List. Core checks those capabilities per
    // selected entry after this selection-only candidate match.
    let copy = excluding_trash(selection("nonzero"));
    let cut = excluding_trash(selection("nonzero"));
    let duplicate = excluding_trash(selection("nonzero"));
    let mut tags = ProviderMatchRule {
        selection: Some("nonzero".into()),
        storage: Some(LocationMatch {
            provider_ids: Some(vec!["core:local-fs".into()]),
            ..Default::default()
        }),
        ..Default::default()
    };
    tags.exclude = Some(exclude_views(&["core:trash"]));

    [
        ("builtin:open", 120, selection("nonzero"), Action::Open),
        ("builtin:open-with", 119, open_with, Action::OpenWith),
        (
            "builtin:preview",
            118,
            excluding_trash(required("single", &[FsOperation::Read])),
            Action::Preview,
        ),
        ("builtin:info", 117, info, Action::Info),
        ("builtin:tags", 116, tags, Action::Tags),
        ("builtin:new-folder", 110, new_folder, Action::NewFolder),
        ("builtin:new-file", 109, new_file, Action::NewFile),
        (
            "builtin:rename",
            106,
            excluding_trash(required("single", &[FsOperation::Rename])),
            Action::Rename,
        ),
        (
            "builtin:batch-rename",
            105,
            excluding_trash(required("multi", &[FsOperation::Rename])),
            Action::BatchRename,
        ),
        ("builtin:copy", 103, copy, Action::Copy),
        ("builtin:cut", 102, cut, Action::Cut),
        ("builtin:duplicate", 101, duplicate, Action::Duplicate),
        ("builtin:copy-path", 100, copy_path, Action::CopyPath),
        ("builtin:pin", 90, storage_vfs("nonzero"), Action::Pin),
        ("builtin:unpin", 89, storage_vfs("nonzero"), Action::Unpin),
        ("builtin:cache", 88, storage_vfs("nonzero"), Action::Cache),
        (
            "builtin:uncache",
            87,
            storage_vfs("nonzero"),
            Action::Uncache,
        ),
        (
            "builtin:conflict-resolution",
            86,
            storage_vfs("nonzero"),
            Action::ConflictResolution,
        ),
        (
            "builtin:move-to-trash",
            10,
            move_to_trash,
            Action::MoveToTrash,
        ),
        (
            "builtin:delete-permanently",
            9,
            delete_permanently,
            Action::DeletePermanently,
        ),
    ]
    .into_iter()
    .map(|(id, priority, rule, action)| ActionDefinition {
        id,
        priority,
        rule,
        action,
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn action_ids_are_unique_and_keep_remove_separate_from_trash() {
        let definitions = definitions();
        let ids = definitions
            .iter()
            .map(|value| value.id)
            .collect::<HashSet<_>>();
        assert_eq!(ids.len(), definitions.len());
        assert_eq!(definitions.len(), 20);
        assert!(definitions.iter().any(
            |value| value.id == "builtin:move-to-trash" && value.action == Action::MoveToTrash
        ));
        assert!(definitions.iter().any(|value| {
            value.id == "builtin:delete-permanently" && value.action == Action::DeletePermanently
        }));
    }
}
