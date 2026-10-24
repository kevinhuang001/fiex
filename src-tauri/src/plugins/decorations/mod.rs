//! Read-only presentation decorations from plugins.
//!
//! A plugin must explicitly declare `contributions.decorations`; plugins that
//! do not are never called. Declarative rules are applied by Core locally.
//!
//! The `types` module holds the serialized value types, `rules` evaluates
//! declarative rules, and `dynamic` invokes plugins, sanitizes their output and
//! merges every provider's contribution.
mod dynamic;
mod rules;
mod types;

pub use dynamic::query;
pub use types::{Decoration, DecorationBadge, DecorationEntry, DecorationField, DecorationStyle};
