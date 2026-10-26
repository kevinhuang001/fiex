//! Read-only presentation decorations from plugins.
//!
//! Decorations are dynamic only. A plugin must declare
//! `extensions.decorations` with an optional `match` filter; Core invokes the
//! standard `decorations.resolve` method, sanitizes the response and merges
//! providers by priority.
mod dynamic;
mod types;

pub use dynamic::query;
pub use types::{Decoration, DecorationBadge, DecorationEntry, DecorationField, DecorationStyle};
