//! Registered Core file-operation menu providers.
use crate::plugins::provider_registry::native::NativeProviderRegistry;
use fiex_provider_contracts::{
    ContextMenuProvider, FsOperation, LocationMatch, MenuItem, ProviderMatchExclude,
    ProviderMatchRule,
};
use std::sync::Arc;

use self::groups::{
    BatchRename, Clipboard, CopyPath, NewFile, NewFolder, Opening, Rename, Tags, Trash,
};
use super::super::connection_cache::ConnectionCache;

mod groups;

fn special_view_exclude() -> ProviderMatchExclude {
    exclude_views(&["core:applications", "core:trash", "core:recent"])
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

/// Every generic Core menu group is registered as the same shared provider
/// trait used by SDK plugins. Native implementations run directly in Core.
pub(super) fn register(values: &mut NativeProviderRegistry<dyn ContextMenuProvider>) {
    let implementations: [(&str, i64, Arc<dyn ContextMenuProvider>); 10] = [
        (
            "builtin:opening",
            100,
            Arc::new(Opening) as Arc<dyn ContextMenuProvider>,
        ),
        (
            "builtin:tags",
            99,
            Arc::new(Tags) as Arc<dyn ContextMenuProvider>,
        ),
        (
            "builtin:new-folder",
            98,
            Arc::new(NewFolder) as Arc<dyn ContextMenuProvider>,
        ),
        (
            "builtin:new-file",
            98,
            Arc::new(NewFile) as Arc<dyn ContextMenuProvider>,
        ),
        (
            "builtin:connection-cache",
            98,
            Arc::new(ConnectionCache) as Arc<dyn ContextMenuProvider>,
        ),
        (
            "builtin:rename",
            96,
            Arc::new(Rename) as Arc<dyn ContextMenuProvider>,
        ),
        (
            "builtin:batch-rename",
            95,
            Arc::new(BatchRename) as Arc<dyn ContextMenuProvider>,
        ),
        (
            "builtin:clipboard",
            94,
            Arc::new(Clipboard) as Arc<dyn ContextMenuProvider>,
        ),
        (
            "builtin:copy-path",
            92,
            Arc::new(CopyPath) as Arc<dyn ContextMenuProvider>,
        ),
        (
            "builtin:trash",
            91,
            Arc::new(Trash) as Arc<dyn ContextMenuProvider>,
        ),
    ];
    for (id, priority, implementation) in implementations {
        let rule = match id {
            "builtin:tags" => ProviderMatchRule {
                storage: Some(LocationMatch {
                    provider_ids: Some(vec!["core:local-fs".into()]),
                    ..Default::default()
                }),
                ..Default::default()
            },
            "builtin:connection-cache" => ProviderMatchRule {
                storage: Some(LocationMatch {
                    types: Some(vec!["vfs".into()]),
                    ..Default::default()
                }),
                ..Default::default()
            },
            "builtin:new-folder" => ProviderMatchRule {
                selection: Some("zero".into()),
                required_ops: vec![FsOperation::Mkdir],
                exclude: Some(exclude_views(&["core:trash", "core:recent"])),
                ..Default::default()
            },
            "builtin:new-file" => ProviderMatchRule {
                selection: Some("zero".into()),
                required_ops: vec![FsOperation::Write],
                exclude: Some(special_view_exclude()),
                ..Default::default()
            },
            "builtin:opening" => ProviderMatchRule {
                exclude: Some(exclude_views(&["core:applications"])),
                ..Default::default()
            },
            "builtin:rename"
            | "builtin:batch-rename"
            | "builtin:clipboard"
            | "builtin:copy-path"
            | "builtin:trash" => ProviderMatchRule {
                exclude: Some(special_view_exclude()),
                ..Default::default()
            },
            _ => ProviderMatchRule::default(),
        };
        values
            .register(id, priority, rule, implementation)
            .expect("invalid builtin menu registration");
    }
}

pub(super) fn item(id: &str, title: &'static str, icon: &str, shortcut: &str) -> MenuItem {
    // Availability decides presence; a builtin item is never greyed out.
    let mut item = MenuItem::action(id, crate::i18n::t(title));
    item.icon = Some(icon.into());
    item.shortcut = (!shortcut.is_empty()).then(|| shortcut.into());
    item
}
