//! Read-only presentation decorations from Core and plugins.
//!
//! Plugins declare `extensions.decorations` with an optional `match` filter.
//! Core invokes the standard `decorations.resolve` method, sanitizes plugin
//! output and merges native and plugin contributions by priority.
mod builtin;
mod dynamic;
mod invocation;
mod native;
mod proxy;
mod query;
mod types;

pub(crate) fn forget_plugin(plugin_id: &str) {
    proxy::forget_plugin(plugin_id);
}

pub use query::{query, query_with_plugins};
pub use types::{Decoration, DecorationBadge, DecorationEntry, DecorationField, DecorationStyle};

#[cfg(test)]
mod tests;
