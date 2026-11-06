//! Registered atomic Core file-operation menu providers.
use crate::plugins::provider_registry::native::NativeProviderRegistry;
use fiex_provider_contracts::{ContextMenuProvider, MenuItem};
use fiex_runtime::ModuleDescriptor;
use fiex_storage::CoreOwnerKey;
use std::sync::Arc;

use groups::AtomicMenuAction;

mod groups;

/// Each registration owns both its match rule and one atomic action. Static
/// eligibility is therefore colocated with the button identity; dynamic
/// availability is computed by that action against the trusted Core snapshot.
pub(super) fn register(values: &mut NativeProviderRegistry<dyn ContextMenuProvider>) {
    let mut module = values.module(ModuleDescriptor::new(
        "core:actions",
        CoreOwnerKey::new("core"),
        env!("CARGO_PKG_VERSION"),
    ));
    for definition in fiex_actions::definitions() {
        module
            .register(
                definition.id,
                definition.priority,
                definition.rule,
                Arc::new(AtomicMenuAction(definition.action)) as Arc<dyn ContextMenuProvider>,
            )
            .expect("invalid builtin menu registration");
    }
}

pub(super) fn item(id: &str, title: &'static str, icon: &str, shortcut: &str) -> MenuItem {
    let mut item = MenuItem::action(id, crate::i18n::t(title));
    item.icon = Some(icon.into());
    item.shortcut = (!shortcut.is_empty()).then(|| shortcut.into());
    item
}
