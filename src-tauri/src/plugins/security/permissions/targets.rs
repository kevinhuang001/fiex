use std::collections::BTreeSet;

use super::model::{declaration_approved, PluginGrant};
use super::store::{mutate, status};

pub fn valid_command(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

pub fn validate_commands(
    declared: &BTreeSet<String>,
    commands: &[String],
) -> Result<BTreeSet<String>, String> {
    if !commands.is_empty() && !declared.contains("commands") {
        return Err("commands must be declared before listing executables".into());
    }
    if commands.len() > 64 {
        return Err("too many requested commands".into());
    }
    let mut unique = BTreeSet::new();
    for command in commands {
        if !valid_command(command) {
            return Err(format!("invalid command name: {command}"));
        }
        unique.insert(command.to_ascii_lowercase());
    }
    Ok(unique)
}

pub fn allowed_command(
    id: &str,
    declared: &BTreeSet<String>,
    command: &str,
) -> Result<bool, String> {
    let command = command.to_ascii_lowercase();
    if !declared.contains("commands") {
        return Ok(false);
    }
    let grant = status(id, declared)?;
    Ok(declaration_approved(&grant, "commands")
        && grant
            .commands
            .iter()
            .any(|granted| granted.eq_ignore_ascii_case(&command)))
}

pub fn normalize_domain(input: &str) -> Result<String, String> {
    let mut domain = input.trim().to_ascii_lowercase();
    if domain.starts_with('[') && domain.ends_with(']') && domain.len() > 2 {
        domain = domain[1..domain.len() - 1].to_string();
    }
    if domain.is_empty() || domain.len() > 253 || !domain.is_ascii() {
        return Err("domain must be a nonempty ASCII hostname or IP address".into());
    }
    if let Ok(address) = domain.parse::<std::net::IpAddr>() {
        return Ok(address.to_string());
    }
    if domain.split('.').any(|label| {
        label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    }) {
        return Err("invalid hostname: enter an exact domain without a wildcard or port".into());
    }
    Ok(domain)
}

/// Classify loopback destinations; this does not grant access to them.
pub fn is_loopback_domain(input: &str) -> bool {
    let Ok(domain) = normalize_domain(input) else {
        return false;
    };
    domain == "localhost"
        || domain
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

pub fn validate_network_domains(
    declared: &BTreeSet<String>,
    domains: &[String],
) -> Result<BTreeSet<String>, String> {
    if domains.len() > 128 || (!domains.is_empty() && !declared.contains("network")) {
        return Err("network domains require a declared network permission (maximum 128)".into());
    }
    domains.iter().map(|d| normalize_domain(d)).collect()
}

/// Only a user action can extend the whitelist. Revoking a domain is immediate
/// for future connections; the plugin process is restarted by the host.
pub fn set_command(
    id: &str,
    declared: &BTreeSet<String>,
    command: &str,
    grant: bool,
) -> Result<PluginGrant, String> {
    if !declared.contains("commands") {
        return Err("plugin did not request command execution".into());
    }
    if !valid_command(command) {
        return Err("invalid command name".into());
    }
    mutate(id, declared, |current| {
        if !current.approved {
            return Err("installation approval required".into());
        }
        if !declaration_approved(current, "commands") {
            return Err("this plugin version's command permission was not approved".into());
        }
        if grant {
            current.commands.insert(command.to_ascii_lowercase());
        } else {
            current.commands.remove(&command.to_ascii_lowercase());
        }
        Ok(())
    })
}

pub fn set_network_all(
    id: &str,
    declared: &BTreeSet<String>,
    all: bool,
) -> Result<PluginGrant, String> {
    if !declared.contains("network") {
        return Err("plugin did not declare network permission".into());
    }
    mutate(id, declared, |current| {
        if !current.approved {
            return Err("installation approval required".into());
        }
        if !declaration_approved(current, "network") {
            return Err("this plugin version's network permission was not approved".into());
        }
        current.network_all = all;
        Ok(())
    })
}

pub fn set_domain(
    id: &str,
    declared: &BTreeSet<String>,
    domain: &str,
    grant: bool,
) -> Result<PluginGrant, String> {
    if !declared.contains("network") {
        return Err("plugin did not declare network permission".into());
    }
    let domain = normalize_domain(domain)?;
    mutate(id, declared, |current| {
        if !current.approved {
            return Err("installation approval required".into());
        }
        if !declaration_approved(current, "network") {
            return Err("this plugin version's network permission was not approved".into());
        }
        if grant {
            if current.network_domains.len() >= 128 {
                return Err("too many allowed domains".into());
            }
            current.network_domains.insert(domain);
        } else {
            current.network_domains.remove(&domain);
        }
        Ok(())
    })
}

pub fn domain_allowed(id: &str, declared: &BTreeSet<String>, domain: &str) -> Result<bool, String> {
    let domain = normalize_domain(domain)?;
    let policy = status(id, declared)?;
    Ok(policy.approved
        && policy.enabled
        && declaration_approved(&policy, "network")
        && policy.granted.contains("network")
        && (policy.network_all || policy.network_domains.contains(&domain)))
}
