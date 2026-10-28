use std::collections::BTreeSet;

use super::model::{declaration_approved, ActivationConsent, PluginGrant};
use super::store::{clear, mutate};
use super::targets::{validate_commands, validate_network_domains};

pub fn revoke_install(id: &str) -> Result<(), String> {
    clear(id)
}
pub fn approve_install(id: &str, declared: &BTreeSet<String>) -> Result<PluginGrant, String> {
    approve_install_with_domains(id, declared, &[], &[])
}

/// Installation consent explicitly covers the requested exact domains and
/// executable names. Plugin processes can never execute commands directly;
/// the host proxy checks this set on every `host.process.run` call.
pub fn approve_install_with_domains(
    id: &str,
    declared: &BTreeSet<String>,
    domains: &[String],
    commands: &[String],
) -> Result<PluginGrant, String> {
    let domains = validate_network_domains(declared, domains)?;
    let commands = validate_commands(declared, commands)?;
    mutate(id, declared, |current| {
        if current.approved {
            return Err("plugin is already authorized".into());
        }
        current.approved = true;
        current.enabled = false;
        // Installation approves this exact declaration snapshot. Every
        // capability still starts denied and must be granted explicitly from
        // plugin settings or an operation-time request.
        current.approved_declared = declared.clone();
        current.granted.clear();
        current.network_domains.clear();
        current.commands.clear();
        let _ = (&domains, &commands);
        Ok(())
    })
}
pub fn set_enabled(
    id: &str,
    declared: &BTreeSet<String>,
    enabled: bool,
) -> Result<PluginGrant, String> {
    mutate(id, declared, |current| {
        if !current.approved {
            return Err("installation approval required".into());
        }
        current.enabled = enabled;
        Ok(())
    })
}

pub fn set_granted(
    id: &str,
    declared: &BTreeSet<String>,
    permission: &str,
    granted: bool,
) -> Result<PluginGrant, String> {
    if !declared.contains(permission) {
        return Err("permission not declared by plugin".into());
    }
    mutate(id, declared, |current| {
        if !current.approved {
            return Err("installation approval required".into());
        }
        if !declaration_approved(current, permission) {
            return Err(
                "this plugin version declares a permission that was not approved; install or approve the update first"
                    .into(),
            );
        }
        if granted {
            current.granted.insert(permission.into());
        } else {
            // Disabling one capability must not stop the plugin: the next
            // operation that needs it asks the user again.
            current.granted.remove(permission);
        }
        Ok(())
    })
}

pub fn activate(
    id: &str,
    required: &ActivationConsent,
    consent: &ActivationConsent,
) -> Result<(), String> {
    // Validate before touching persistent grants; consent is also the package
    // approval for packages installed by copying their directory.
    validate_consent(required, consent)?;
    mutate(id, &required.permissions, |current| {
        apply_consent(current, required, consent)
    })?;
    Ok(())
}

fn validate_consent(
    required: &ActivationConsent,
    consent: &ActivationConsent,
) -> Result<(), String> {
    // The user may deselect declared items, but the selection can never grow
    // beyond what the installed manifest (and its approved snapshot) declares.
    if !consent.is_subset_of(required) {
        return Err(
            "permission requirements changed or consent includes undeclared items; review the list again"
                .into(),
        );
    }
    Ok(())
}

fn apply_consent(
    current: &mut PluginGrant,
    required: &ActivationConsent,
    consent: &ActivationConsent,
) -> Result<(), String> {
    validate_consent(required, consent)?;
    // This explicit, manifest-validated consent is also the installation
    // approval for packages copied directly into the plugin directory.
    current.approved_declared = required.permissions.clone();
    current.approved = true;
    // Store only what the user selected. Deselected capabilities stay
    // ungranted and are requested again if an operation needs them.
    current.granted = consent.permissions.clone();
    current.network_domains = consent.domains.clone();
    current.network_all = consent.network_all;
    current.commands = consent.commands.clone();
    current.enabled = true;
    Ok(())
}

#[cfg(test)]
mod activation_tests {
    use super::*;
    #[test]
    fn copied_packages_require_explicit_valid_consent() {
        let required = ActivationConsent {
            permissions: ["files.read".into(), "network".into()].into(),
            domains: ["example.com".into()].into(),
            ..Default::default()
        };
        let selected = ActivationConsent {
            permissions: ["files.read".into()].into(),
            ..Default::default()
        };
        let mut current = PluginGrant::default();
        assert!(!current.approved && !current.enabled);
        apply_consent(&mut current, &required, &selected).unwrap();
        assert!(current.approved && current.enabled);
        assert_eq!(current.granted, selected.permissions);
        assert!(current.network_domains.is_empty());
        let invalid = ActivationConsent {
            permissions: ["files.write".into()].into(),
            ..Default::default()
        };
        assert!(apply_consent(&mut current, &required, &invalid).is_err());
        assert_eq!(current.granted, selected.permissions);
    }
}
