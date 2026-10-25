//! Central permission decisions for plugin capabilities.
//!
//! The grant store lives in `plugin_permissions`; this module is the single
//! place that turns a plugin's declared permissions and the user's grant into
//! allow/deny decisions. Brokers and RPC handlers must ask this policy rather
//! than interpreting permission strings themselves.
use crate::plugin_permissions::{self, PluginGrant};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capability {
    Metadata,
    FoldersRead,
    FoldersCreate,
    FoldersDelete,
    FoldersRename,
    FilesRead,
    FilesWrite,
    FilesCreate,
    FilesDelete,
    FilesRename,
    Network,
    Commands,
    Hooks,
}

impl Capability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Metadata => "metadata",
            Self::FoldersRead => "folders.read",
            Self::FoldersCreate => "folders.create",
            Self::FoldersDelete => "folders.delete",
            Self::FoldersRename => "folders.rename",
            Self::FilesRead => "files.read",
            Self::FilesWrite => "files.write",
            Self::FilesCreate => "files.create",
            Self::FilesDelete => "files.delete",
            Self::FilesRename => "files.rename",
            Self::Network => "network",
            Self::Commands => "commands",
            Self::Hooks => "hooks",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "metadata" => Self::Metadata,
            "folders.read" => Self::FoldersRead,
            "folders.create" => Self::FoldersCreate,
            "folders.delete" => Self::FoldersDelete,
            "folders.rename" => Self::FoldersRename,
            "files.read" => Self::FilesRead,
            "files.write" => Self::FilesWrite,
            "files.create" => Self::FilesCreate,
            "files.delete" => Self::FilesDelete,
            "files.rename" => Self::FilesRename,
            "network" => Self::Network,
            "commands" => Self::Commands,
            "hooks" => Self::Hooks,
            _ => return None,
        })
    }
}

/// A permission decision for one plugin instance. Decision failures are closed:
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

    pub fn allowed(&self, capability: Capability) -> bool {
        if !self.declared.contains(capability.as_str()) {
            return false;
        }
        plugin_permissions::allowed(self.plugin_id, self.declared, capability.as_str())
            .unwrap_or(false)
    }

    pub fn allowed_name(&self, name: &str) -> bool {
        Capability::from_name(name).is_some_and(|capability| self.allowed(capability))
    }

    pub fn require(&self, capability: Capability) -> Result<(), String> {
        if self.allowed(capability) {
            Ok(())
        } else {
            Err(format!(
                "PermissionDenied: {} is not granted",
                capability.as_str()
            ))
        }
    }

    pub fn require_all(&self) -> Result<(), String> {
        for permission in self.declared {
            if !Capability::from_name(permission).is_some_and(|capability| self.allowed(capability))
            {
                return Err(format!("PermissionDenied: {permission} is not granted"));
            }
        }
        Ok(())
    }

    pub fn domain_allowed(&self, host: &str) -> Result<bool, String> {
        if !self.allowed(Capability::Network) {
            return Ok(false);
        }
        plugin_permissions::domain_allowed(self.plugin_id, self.declared, host)
    }

    pub fn command_allowed(&self, command: &str) -> Result<bool, String> {
        if !self.allowed(Capability::Commands) {
            return Ok(false);
        }
        plugin_permissions::allowed_command(self.plugin_id, self.declared, command)
    }

    pub fn active(&self) -> Result<bool, String> {
        let grant = self.grant()?;
        Ok(grant.approved
            && grant.enabled
            && self
                .declared
                .iter()
                .all(|permission| grant.granted.contains(permission)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_names_round_trip() {
        for capability in [
            Capability::Metadata,
            Capability::FilesRead,
            Capability::Network,
            Capability::Commands,
            Capability::Hooks,
        ] {
            assert_eq!(Capability::from_name(capability.as_str()), Some(capability));
        }
        assert_eq!(Capability::from_name("unknown"), None);
    }
}
