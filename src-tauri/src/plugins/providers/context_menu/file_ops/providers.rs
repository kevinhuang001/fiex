//! Registered atomic Core file-operation menu providers.
use crate::plugins::provider_registry::native::NativeProviderRegistry;
use fiex_provider_contracts::{
    ContextMenuProvider, FsOperation, LocationMatch, ProviderMatchExclude, ProviderMatchRule,
};
use fiex_runtime::ModuleDescriptor;
use fiex_storage::CoreOwnerKey;
use std::sync::Arc;

use groups::{Action, AtomicMenuAction};

mod groups;

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

/// Each registration owns both its match rule and one atomic action. Static
/// eligibility is therefore colocated with the button identity; dynamic
/// availability is computed by that action against the trusted Core snapshot.
pub(super) fn register(values: &mut NativeProviderRegistry<dyn ContextMenuProvider>) {
    let mut new_folder = required("zero", &[FsOperation::Mkdir]);
    new_folder.exclude = Some(exclude_views(&["core:trash", "core:recent"]));
    let mut new_file = required("zero", &[FsOperation::Write]);
    new_file.exclude = Some(exclude_views(&[
        "core:applications",
        "core:trash",
        "core:recent",
    ]));
    // Recent is a projection whose own VFS contract only grants stat/read.
    // Keep deletion attached to the projection's authorization, never the
    // backing file's broader capabilities.
    let mut move_to_trash = required("nonzero", &[FsOperation::Trash]);
    move_to_trash.exclude = Some(exclude_views(&["core:recent", "core:trash"]));
    let mut delete_permanently = required("nonzero", &[FsOperation::Remove]);
    delete_permanently.exclude = Some(exclude_views(&["core:recent"]));
    let mut info = selection("single");
    info.exclude = Some(exclude_views(&["core:trash", "core:applications"]));
    let mut copy_path = selection("nonzero");
    copy_path.exclude = Some(exclude_views(&["core:trash", "core:applications"]));
    let mut tags = ProviderMatchRule {
        selection: Some("nonzero".into()),
        storage: Some(LocationMatch {
            provider_ids: Some(vec!["core:local-fs".into()]),
            ..Default::default()
        }),
        ..Default::default()
    };
    tags.exclude = Some(exclude_views(&["core:trash"]));
    let actions = [
        ("builtin:open", 120, selection("nonzero"), Action::Open),
        (
            "builtin:preview",
            119,
            excluding_trash(required("single", &[FsOperation::Read])),
            Action::Preview,
        ),
        ("builtin:info", 118, info, Action::Info),
        ("builtin:tags", 117, tags, Action::Tags),
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
        (
            "builtin:copy",
            103,
            excluding_trash(required("nonzero", &[FsOperation::Read])),
            Action::Copy,
        ),
        (
            "builtin:cut",
            102,
            excluding_trash(required(
                "nonzero",
                &[FsOperation::Read, FsOperation::Remove],
            )),
            Action::Cut,
        ),
        (
            "builtin:duplicate",
            101,
            excluding_trash(required(
                "nonzero",
                &[FsOperation::Read, FsOperation::Write],
            )),
            Action::Duplicate,
        ),
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
    ];
    let mut module = values.module(ModuleDescriptor::new(
        "core:actions",
        CoreOwnerKey::new("core"),
        env!("CARGO_PKG_VERSION"),
    ));
    for (id, priority, rule, action) in actions {
        module
            .register(
                id,
                priority,
                rule,
                Arc::new(AtomicMenuAction(action)) as Arc<dyn ContextMenuProvider>,
            )
            .expect("invalid builtin menu registration");
    }
}

pub(super) fn item(
    id: &str,
    title: &'static str,
    icon: &str,
    shortcut: &str,
) -> fiex_provider_contracts::MenuItem {
    let mut item = fiex_provider_contracts::MenuItem::action(id, crate::i18n::t(title));
    item.icon = Some(icon.into());
    item.shortcut = (!shortcut.is_empty()).then(|| shortcut.into());
    item
}
