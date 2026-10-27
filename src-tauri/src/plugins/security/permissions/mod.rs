//! Host-authorized plugin capabilities.
//!
//! The user-facing permission model is deliberately small:
//! - `metadata`: read attributes of the selected file or directory;
//! - `folders.read`: list directories within the approved scope;
//! - `folders.create`, `folders.delete`, `folders.rename`: independent entry operations;
//! - `files.read`: read file contents;
//! - `files.write`: modify file contents;
//! - `network`: network targets explicitly allowed by the user;
//! - `commands`: explicitly approved executable names, run by Host.
//!
//! Private plugin storage and host-rendered choosers are brokered by the host.
//! Grants are only one layer: `plugin_permission_policy` combines them with the
//! manifest declaration and the operation scope. This module stores state and
//! never decides whether a concrete resource is reachable.
mod grants;
mod model;
mod store;
mod targets;
#[cfg(test)]
mod tests;

pub use grants::{
    activate, approve_install, approve_install_with_domains, revoke_install, set_enabled,
    set_granted,
};
pub use model::{
    activation_complete, declaration_approved, ActivationConsent, PluginGrant, PERMISSIONS,
};
pub use store::{allowed, status};
pub use targets::{
    allowed_command, domain_allowed, is_loopback_domain, normalize_domain, set_command, set_domain,
    set_network_all, valid_command, validate_commands, validate_network_domains,
};
