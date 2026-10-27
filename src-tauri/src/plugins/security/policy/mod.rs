//! Three-layer permission decisions: manifest declaration, user state and
//! operation scope. Scope itself is validated by the caller before it asks for
//! a decision, so out-of-scope requests never become prompts.
mod capability;
mod decision;
mod evaluation;
#[cfg(test)]
mod tests;

pub use capability::Capability;
pub use decision::{decide, Decision, DenyReason};
pub use evaluation::PermissionPolicy;
