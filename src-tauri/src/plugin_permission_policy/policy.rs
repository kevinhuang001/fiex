use std::collections::BTreeSet;

use crate::plugin_permissions::{self, PluginGrant};

use super::capability::Capability;
use super::decision::{decide, decide_command, decide_domain, Decision};

/// A permission view for one plugin instance. Decision failures are closed:
/// an unreadable policy store, an unknown capability name, or a revoked grant
/// all deny.
pub struct PermissionPolicy<'a> {
    plugin_id: &'a str,
    declared: &'a BTreeSet<String>,
}

impl<'a> PermissionPolicy<'a> {
    pub fn new(plugin_id: &'a str, declared: &'a BTreeSet<String>) -> Self {
        Self {
            plugin_id,
            declared,
        }
    }

    pub fn plugin_id(&self) -> &str {
        self.plugin_id
    }

    pub fn declared(&self) -> &BTreeSet<String> {
        self.declared
    }

    pub fn grant(&self) -> Result<PluginGrant, String> {
        plugin_permissions::status(self.plugin_id, self.declared)
    }

    /// Check only the manifest and user-state layers. The caller must have
    /// validated the operation scope already.
    pub fn check(&self, capability: Capability) -> Result<Decision, String> {
        let grant = self.grant()?;
        Ok(decide(self.declared, &grant, capability))
    }

    pub fn allowed(&self, capability: Capability) -> bool {
        matches!(self.check(capability), Ok(Decision::Allowed))
    }

    pub fn allowed_name(&self, name: &str) -> bool {
        Capability::from_name(name).is_some_and(|capability| self.allowed(capability))
    }

    pub fn require(&self, capability: Capability) -> Result<(), String> {
        match self.check(capability)? {
            Decision::Allowed => Ok(()),
            Decision::Prompt => Err(format!(
                "PermissionDenied: {} requires user consent",
                capability.as_str()
            )),
            Decision::Denied(reason) => Err(reason.message(capability)),
        }
    }

    pub fn require_all(&self) -> Result<(), String> {
        for permission in self.declared {
            let capability = Capability::from_name(permission)
                .ok_or_else(|| format!("PermissionDenied: unknown permission {permission}"))?;
            self.require(capability)?;
        }
        Ok(())
    }

    pub fn domain_check(&self, host: &str) -> Result<Decision, String> {
        let grant = self.grant()?;
        decide_domain(self.declared, &grant, host)
    }

    pub fn domain_allowed(&self, host: &str) -> Result<bool, String> {
        Ok(matches!(self.domain_check(host)?, Decision::Allowed))
    }

    pub fn command_check(&self, command: &str) -> Result<Decision, String> {
        let grant = self.grant()?;
        Ok(decide_command(self.declared, &grant, command))
    }

    pub fn command_allowed(&self, command: &str) -> Result<bool, String> {
        Ok(matches!(self.command_check(command)?, Decision::Allowed))
    }

    /// Installed, approved and enabled. Individual capabilities may still be
    /// switched off; those are handled by `check` and can be requested again.
    pub fn active(&self) -> Result<bool, String> {
        let grant = self.grant()?;
        Ok(grant.approved && grant.enabled)
    }
}
