use crate::plugins::security::permissions::{self as plugin_permissions, PluginGrant};
use std::collections::BTreeSet;

use super::capability::Capability;

/// Why an operation must be denied without asking the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DenyReason {
    /// The installed manifest never declared this capability. A plugin cannot
    /// obtain it by asking at runtime; it needs a new signed release and an
    /// install/upgrade review.
    NotDeclared,
    /// The package itself has not been approved.
    NotApproved,
    /// The user disabled the plugin rather than one capability.
    Disabled,
    /// The manifest on disk declares more than the approved snapshot. This is
    /// treated like an unapproved upgrade: no prompt, no runtime grant.
    DeclarationNotApproved,
}

impl DenyReason {
    pub fn message(self, capability: Capability) -> String {
        match self {
            Self::NotDeclared => format!(
                "PermissionDenied: {} is not declared by this plugin",
                capability.as_str()
            ),
            Self::NotApproved => "PermissionDenied: installation approval is required".into(),
            Self::Disabled => "PermissionDenied: plugin is disabled".into(),
            Self::DeclarationNotApproved => format!(
                "PermissionDenied: {} was added without approving this plugin update",
                capability.as_str()
            ),
        }
    }
}

/// The outcome of the manifest + user-state layers. Scope is checked by the
/// caller before it acts on this value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Declared, approved, active and currently granted.
    Allowed,
    /// Declared and approved, but the user switched this capability off. Core
    /// may show one coalesced prompt for the current operation.
    Prompt,
    /// Deny immediately; showing a prompt would let a plugin ask for something
    /// it is not allowed to obtain.
    Denied(DenyReason),
}

/// Pure three-layer decision for a capability name. Scope is deliberately not
/// part of this function: callers must validate the operation scope first so
/// out-of-scope requests are denied without a prompt.
pub fn decide(
    declared: &BTreeSet<String>,
    grant: &PluginGrant,
    capability: Capability,
) -> Decision {
    let name = capability.as_str();
    if !declared.contains(name) {
        return Decision::Denied(DenyReason::NotDeclared);
    }
    if !grant.approved {
        return Decision::Denied(DenyReason::NotApproved);
    }
    if !grant.enabled {
        return Decision::Denied(DenyReason::Disabled);
    }
    if !plugin_permissions::declaration_approved(grant, name) {
        return Decision::Denied(DenyReason::DeclarationNotApproved);
    }
    if grant.granted.contains(name) {
        Decision::Allowed
    } else {
        Decision::Prompt
    }
}

pub(super) fn decide_domain(
    declared: &BTreeSet<String>,
    grant: &PluginGrant,
    host: &str,
) -> Result<Decision, String> {
    let host = plugin_permissions::normalize_domain(host)?;
    Ok(match decide(declared, grant, Capability::Network) {
        Decision::Allowed if grant.network_all || grant.network_domains.contains(&host) => {
            Decision::Allowed
        }
        Decision::Allowed | Decision::Prompt => Decision::Prompt,
        denied => denied,
    })
}

pub(super) fn decide_command(
    declared: &BTreeSet<String>,
    grant: &PluginGrant,
    command: &str,
) -> Decision {
    let command = command.to_ascii_lowercase();
    match decide(declared, grant, Capability::Commands) {
        Decision::Allowed
            if grant
                .commands
                .iter()
                .any(|granted| granted.eq_ignore_ascii_case(&command)) =>
        {
            Decision::Allowed
        }
        Decision::Allowed | Decision::Prompt => Decision::Prompt,
        denied => denied,
    }
}
