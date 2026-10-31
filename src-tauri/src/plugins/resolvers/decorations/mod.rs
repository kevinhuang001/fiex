//! Read-only presentation decorations from plugins.
//!
//! Decorations are dynamic only. A plugin must declare
//! `extensions.decorations` with an optional `match` filter; Core invokes the
//! standard `decorations.resolve` method, sanitizes the response and merges
//! providers by priority.
mod builtin;
mod endpoint;
mod dynamic;
mod query;
mod types;

pub use query::{query, query_with_plugins};
pub use types::{Decoration, DecorationBadge, DecorationEntry, DecorationField, DecorationStyle};

#[cfg(test)]
mod tests;
