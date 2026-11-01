//! Feature resolvers share the EventRouter; each owns its result policy.
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
