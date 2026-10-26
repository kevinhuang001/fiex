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
    /// `None` is a legacy record: it is migrated from the current declaration
    /// once, because installation approval always covered the whole manifest.
    #[serde(default)]
    pub approved_declared: Option<BTreeSet<String>>,
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
    grant.approved
        && grant
            .approved_declared
            .as_ref()
            .is_some_and(|approved| approved.contains(permission))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ActivationConsent {
    pub permissions: BTreeSet<String>,
    pub network_all: bool,
    pub domains: BTreeSet<String>,
    pub commands: BTreeSet<String>,
}

pub fn activation_complete(grant: &PluginGrant, required: &ActivationConsent) -> bool {
    grant.approved
        && required.permissions.is_subset(&grant.granted)
        && (!required.network_all || grant.network_all)
        && required.domains.is_subset(&grant.network_domains)
        && required.commands.is_subset(&grant.commands)
}
