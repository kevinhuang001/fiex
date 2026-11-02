//! Registered Core file-operation menu providers.
use crate::plugins::provider_registry::native::NativeProviderRegistry;
use fiex_provider_contracts::{ContextMenuProvider, LocationMatch, MenuItem, ProviderMatchRule};
use std::sync::Arc;

use self::groups::{
    BatchRename, Clipboard, CopyPath, Create, Opening, Rename, Tags, Terminal, Trash,
};
use super::super::connection_cache::ConnectionCache;

mod groups;

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
            "builtin:create",
            98,
            Arc::new(Create) as Arc<dyn ContextMenuProvider>,
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
            "builtin:terminal",
            93,
            Arc::new(Terminal) as Arc<dyn ContextMenuProvider>,
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
            "builtin:tags" | "builtin:terminal" => ProviderMatchRule {
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
