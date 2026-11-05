//! Feature providers share the EventRouter; each feature owns its result policy.
pub mod connection;
pub mod connection_menu;
pub mod context_menu;
pub mod decorations;
pub mod fs;
pub mod hooks;
pub mod info;
pub mod preview;
pub mod search;
pub mod toolbar;

use std::sync::OnceLock;

/// Register every Core-owned implementation into the shared Runtime once.
/// Provider-kind modules contribute through the same typed ModuleRegistrar;
/// their request paths only create lightweight typed views over that registry.
pub(crate) fn initialize_native_providers() {
    static INITIALIZED: OnceLock<()> = OnceLock::new();
    INITIALIZED.get_or_init(register_native_providers);
}

fn register_native_providers() {
    use crate::plugins::provider_registry::native::NativeProviderRegistry;
    use fiex_provider_contracts::{
        ConnectionContextMenuProvider, ContextMenuProvider, DecorationProvider, HookProvider,
        InfoProvider, PreviewProvider, ProviderKind, SearchProvider,
    };

    let mut vfs = NativeProviderRegistry::shared(ProviderKind::Vfs);
    crate::core::volumes::native_vfs::register(&mut vfs);

    let mut hook_registry: NativeProviderRegistry<dyn HookProvider> =
        NativeProviderRegistry::shared(ProviderKind::Hook);
    hooks::register_native(&mut hook_registry);

    let mut info_registry: NativeProviderRegistry<dyn InfoProvider> =
        NativeProviderRegistry::shared(ProviderKind::Info);
    info::register_native(&mut info_registry);

    let mut previews: NativeProviderRegistry<dyn PreviewProvider> =
        NativeProviderRegistry::shared(ProviderKind::Preview);
    preview::register_native(&mut previews);

    let mut search_registry: NativeProviderRegistry<dyn SearchProvider> =
        NativeProviderRegistry::shared(ProviderKind::Search);
    search::register_native(&mut search_registry);

    let mut decoration_registry: NativeProviderRegistry<dyn DecorationProvider> =
        NativeProviderRegistry::shared(ProviderKind::Decoration);
    decorations::register_native(&mut decoration_registry);

    let mut menu_registry: NativeProviderRegistry<dyn ContextMenuProvider> =
        NativeProviderRegistry::shared(ProviderKind::ContextMenu);
    context_menu::register_native(&mut menu_registry);

    let mut connection_menus: NativeProviderRegistry<dyn ConnectionContextMenuProvider> =
        NativeProviderRegistry::shared(ProviderKind::ConnectionContextMenu);
    connection_menu::register_native(&mut connection_menus);
}

/// Drop Core-cached IPC proxy instances when a plugin is uninstalled. Existing
/// in-flight `Arc` handles finish naturally; subsequent lookups register a new
/// proxy only after the plugin is installed and authorized again.
pub(crate) fn forget_plugin(plugin_id: &str) {
    connection::forget_plugin(plugin_id);
    connection_menu::forget_plugin(plugin_id);
    context_menu::forget_plugin(plugin_id);
    decorations::forget_plugin(plugin_id);
    fs::forget_plugin(plugin_id);
    hooks::forget_plugin(plugin_id);
    info::forget_plugin(plugin_id);
    preview::forget_plugin(plugin_id);
    search::forget_plugin(plugin_id);
    toolbar::forget_plugin(plugin_id);
}

#[cfg(test)]
mod tests {
    use super::initialize_native_providers;
    use crate::plugins::provider_registry::native::NativeProviderRegistry;
    use fiex_provider_contracts::{
        ConnectionContextMenuProvider, ContextMenuProvider, DecorationProvider, HookProvider,
        InfoProvider, PreviewProvider, ProviderKind, SearchProvider,
    };

    #[test]
    fn central_bootstrap_registers_all_core_provider_kinds() {
        initialize_native_providers();

        assert!(NativeProviderRegistry::<
            dyn crate::core::volumes::native_vfs::RegisteredNativeVfsProvider,
        >::shared(ProviderKind::Vfs)
        .get("core:recent")
        .is_some());
        assert!(
            NativeProviderRegistry::<dyn HookProvider>::shared(ProviderKind::Hook)
                .get("core:recent.hook")
                .is_some()
        );
        assert!(
            NativeProviderRegistry::<dyn InfoProvider>::shared(ProviderKind::Info)
                .get("core.metadata")
                .is_some()
        );
        assert!(
            NativeProviderRegistry::<dyn PreviewProvider>::shared(ProviderKind::Preview)
                .get("builtin:fallback")
                .is_some()
        );
        assert!(
            NativeProviderRegistry::<dyn SearchProvider>::shared(ProviderKind::Search)
                .get("core.search")
                .is_some()
        );
        assert!(
            NativeProviderRegistry::<dyn DecorationProvider>::shared(ProviderKind::Decoration)
                .get("builtin:tags")
                .is_some()
        );
        assert!(NativeProviderRegistry::<dyn ContextMenuProvider>::shared(
            ProviderKind::ContextMenu
        )
        .get("builtin:open")
        .is_some());
        assert!(
            NativeProviderRegistry::<dyn ConnectionContextMenuProvider>::shared(
                ProviderKind::ConnectionContextMenu
            )
            .get("builtin:connection-edit")
            .is_some()
        );
    }
}
