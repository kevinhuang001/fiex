use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const PERMISSIONS: &[&str] = &[
    "metadata",
    "folders.read",
    "folders.create",
    "folders.delete",
    "folders.rename",
    "files.read",
    "files.write",
    "files.create",
    "files.delete",
    "files.rename",
    "network",
    "commands",
];

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginGrant {
    #[serde(default)]
    pub approved: bool,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub granted: BTreeSet<String>,
    /// The declaration snapshot approved with the installed package. Runtime
    /// grants can never exceed this set, even if the manifest on disk changes.
    #[serde(default)]
    pub approved_declared: BTreeSet<String>,
    /// Exact outbound hostnames approved by the user. No wildcards.
    #[serde(default)]
    pub network_domains: BTreeSet<String>,
    /// Dangerous opt-in: any hostname, not just `network_domains`.
    #[serde(default)]
    pub network_all: bool,
    #[serde(default)]
    pub commands: BTreeSet<String>,
}

pub fn declaration_approved(grant: &PluginGrant, permission: &str) -> bool {
    grant.approved && grant.approved_declared.contains(permission)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ActivationConsent {
    pub permissions: BTreeSet<String>,
    pub network_all: bool,
    pub domains: BTreeSet<String>,
    pub commands: BTreeSet<String>,
}

impl ActivationConsent {
    /// Activation is partial by design: the user may deselect any declared
    /// capability or target before activating, but may never select something
    /// the manifest did not request. `networkAll` is only selectable when the
    /// manifest asked for it, and a target is meaningless without the
    /// capability that reaches it.
    pub fn is_subset_of(&self, required: &Self) -> bool {
        if !self.permissions.is_subset(&required.permissions)
            || (self.network_all && !required.network_all)
            || !self.domains.is_subset(&required.domains)
            || !self.commands.is_subset(&required.commands)
        {
            return false;
        }
        if (self.network_all || !self.domains.is_empty()) && !self.permissions.contains("network") {
            return false;
        }
        if !self.commands.is_empty() && !self.permissions.contains("commands") {
            return false;
        }
        true
    }
}

/// Whether the stored grant reflects the consent the user actually gave.
/// Declared capabilities that were deselected are not required: activation
/// stores only the selected subset.
pub fn activation_complete(grant: &PluginGrant, consent: &ActivationConsent) -> bool {
    grant.approved
        && consent.permissions.is_subset(&grant.granted)
        && (!consent.network_all || grant.network_all)
        && consent.domains.is_subset(&grant.network_domains)
        && consent.commands.is_subset(&grant.commands)
}
