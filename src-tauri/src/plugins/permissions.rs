//! Host-authorized plugin capabilities.
//!
//! The user-facing permission model is deliberately small:
//! - `folders.read`: list directories and read entry metadata (including decoration data);
//! - `folders.write`: create, remove and rename directory entries;
//! - `files.read`: read file contents;
//! - `files.write`: modify file contents;
//! - `network`: open network connections to explicitly allowed domains.
//!
//! Private plugin storage, host-rendered file choosers and process execution
//! are not permission items. Storage and choosers are brokered by the host;
//! execution is constrained by the OS sandbox.
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

pub const PERMISSIONS: &[&str] = &[
    "folders.read",
    "folders.create",
    "folders.delete",
    "folders.rename",
    "files.read",
    "files.write",
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
    /// Exact outbound hostnames approved by the user. No wildcards.
    #[serde(default)]
    pub network_domains: BTreeSet<String>,
    #[serde(default)]
    pub commands: BTreeSet<String>,
}
#[derive(Default, Serialize, Deserialize)]
struct PolicyStore {
    #[serde(default)]
    plugins: BTreeMap<String, PluginGrant>,
}
fn gate() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}
fn path() -> Result<PathBuf, String> {
    dirs::data_local_dir()
        .map(|p| p.join("fiex").join("plugin-permissions.json"))
        .ok_or("application data directory unavailable".into())
}
fn load() -> Result<PolicyStore, String> {
    match fs::read(path()?) {
        Ok(data) => {
            serde_json::from_slice(&data).map_err(|e| format!("permission store unavailable: {e}"))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(PolicyStore::default()),
        Err(e) => Err(e.to_string()),
    }
}
fn save(state: &PolicyStore) -> Result<(), String> {
    let target = path()?;
    let parent = target.parent().ok_or("invalid permission storage path")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec_pretty(state).map_err(|e| e.to_string())?;
    let staging = parent.join(format!("plugin-permissions-{}.tmp", std::process::id()));
    let result = (|| {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)
            .map_err(|e| e.to_string())?;
        f.write_all(&bytes).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        #[cfg(windows)]
        if target.exists() {
            fs::remove_file(&target).map_err(|e| e.to_string())?;
        }
        fs::rename(&staging, &target).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&staging);
    }
    result
}
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
    Ok(status(id, declared)?
        .commands
        .iter()
        .any(|granted| granted.eq_ignore_ascii_case(&command)))
}

fn effective(record: Option<&PluginGrant>, declared: &BTreeSet<String>) -> PluginGrant {
    match record {
        Some(r) if r.approved => PluginGrant {
            approved: true,
            enabled: r.enabled,
            granted: r.granted.intersection(declared).cloned().collect(),
            network_domains: if declared.contains("network") {
                r.network_domains
                    .iter()
                    .filter(|d| normalize_domain(d).is_ok())
                    .cloned()
                    .collect()
            } else {
                BTreeSet::new()
            },
            commands: if declared.contains("commands") {
                r.commands
                    .iter()
                    .filter(|command| valid_command(command))
                    .cloned()
                    .collect()
            } else {
                BTreeSet::new()
            },
        },
        _ => PluginGrant::default(),
    }
}
pub fn status(id: &str, declared: &BTreeSet<String>) -> Result<PluginGrant, String> {
    let _guard = gate()
        .lock()
        .map_err(|_| "permission store lock poisoned")?;
    Ok(effective(load()?.plugins.get(id), declared))
}
pub fn allowed(id: &str, declared: &BTreeSet<String>, permission: &str) -> Result<bool, String> {
    if !declared.contains(permission) {
        return Ok(false);
    }
    let grant = status(id, declared)?;
    Ok(grant.approved && grant.enabled && grant.granted.contains(permission))
}

/// Normalize an exact DNS hostname, IPv4 or IPv6 address. Bracketed IPv6
/// literals are accepted. Schemas, paths, ports, wildcards and leading dots
/// are not permission scopes.
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

/// `localhost` and loopback literals never need an explicit allowlist entry.
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
        if grant {
            current.commands.insert(command.to_ascii_lowercase());
        } else {
            current.commands.remove(&command.to_ascii_lowercase());
            current.enabled = false;
        }
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
        if grant {
            if current.network_domains.len() >= 128 {
                return Err("too many allowed domains".into());
            }
            current.network_domains.insert(domain);
        } else {
            current.network_domains.remove(&domain);
            current.enabled = false;
        }
        Ok(())
    })
}

pub fn domain_allowed(id: &str, declared: &BTreeSet<String>, domain: &str) -> Result<bool, String> {
    let domain = normalize_domain(domain)?;
    let policy = status(id, declared)?;
    Ok(policy.approved
        && policy.enabled
        && policy.granted.contains("network")
        && policy.network_domains.contains(&domain))
}

pub fn revoke_install(id: &str) -> Result<(), String> {
    let _guard = gate().lock().map_err(|_| "permission store locked")?;
    let mut state = load()?;
    state.plugins.insert(id.to_owned(), PluginGrant::default());
    save(&state)
}
pub fn approve_install(id: &str, declared: &BTreeSet<String>) -> Result<PluginGrant, String> {
    approve_install_with_domains(id, declared, &[], &[])
}

/// Installation consent explicitly covers the requested exact domains and
/// executable names. Plugin processes can never execute commands directly;
/// the host proxy checks this set on every `host.exec` call.
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
        // Installation only approves the package. Every capability starts
        // denied and must be granted explicitly from plugin settings.
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
        if granted {
            current.granted.insert(permission.into());
        } else {
            current.granted.remove(permission);
            current.enabled = false;
        }
        Ok(())
    })
}
fn mutate(
    id: &str,
    declared: &BTreeSet<String>,
    f: impl FnOnce(&mut PluginGrant) -> Result<(), String>,
) -> Result<PluginGrant, String> {
    let _guard = gate()
        .lock()
        .map_err(|_| "permission store lock poisoned")?;
    let mut store = load()?;
    let mut next = effective(store.plugins.get(id), declared);
    f(&mut next)?;
    store.plugins.insert(id.into(), next.clone());
    save(&store)?;
    Ok(next)
}

/// A complete activation snapshot, reviewed item by item by the user.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ActivationConsent {
    pub permissions: BTreeSet<String>,
    pub domains: BTreeSet<String>,
    pub commands: BTreeSet<String>,
}

pub fn activation_complete(grant: &PluginGrant, required: &ActivationConsent) -> bool {
    grant.approved
        && required.permissions.is_subset(&grant.granted)
        && required.domains.is_subset(&grant.network_domains)
        && required.commands.is_subset(&grant.commands)
}

pub fn activate(
    id: &str,
    required: &ActivationConsent,
    consent: &ActivationConsent,
) -> Result<(), String> {
    if consent != required {
        return Err(
            "permission requirements changed or consent is incomplete; review every item again"
                .into(),
        );
    }
    mutate(id, &required.permissions, |current| {
        current.approved = true;
        current.granted = required.permissions.clone();
        current.network_domains = required.domains.clone();
        current.commands = required.commands.clone();
        current.enabled = true;
        Ok(())
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn activation_requires_every_capability_and_target() {
        let required = ActivationConsent {
            permissions: ["files.read".into(), "commands".into(), "network".into()].into(),
            commands: ["tool".into()].into(),
            domains: ["example.com".into()].into(),
        };
        let mut grant = PluginGrant {
            approved: true,
            granted: required.permissions.clone(),
            ..Default::default()
        };
        assert!(!activation_complete(&grant, &required));
        grant.commands = required.commands.clone();
        grant.network_domains = required.domains.clone();
        assert!(activation_complete(&grant, &required));
        grant.granted.remove("files.read");
        assert!(!activation_complete(&grant, &required));
    }

    fn declared(values: &[&str]) -> BTreeSet<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }
    #[test]
    fn all_plugins_start_unapproved() {
        let requested = declared(&["filesystem.read", "filesystem.write", "network"]);
        let state = effective(None, &requested);
        assert!(!state.approved && !state.enabled && state.granted.is_empty());
    }
    #[test]
    fn removed_grants_stay_removed_and_new_scopes_never_auto_grant() {
        let old = declared(&["filesystem.read"]);
        let current = PluginGrant {
            approved: true,
            enabled: true,
            granted: old,
            network_domains: BTreeSet::new(),
            commands: BTreeSet::new(),
        };
        let new = declared(&["filesystem.read", "filesystem.write"]);
        let s = effective(Some(&current), &new);
        assert!(s.granted.contains("filesystem.read") && !s.granted.contains("filesystem.write"));
    }
    #[test]
    fn command_grants_are_exact_and_require_declaration() {
        let commands = declared(&["commands"]);
        assert!(valid_command("git"));
        assert!(valid_command("python3"));
        for value in ["", "../git", "/usr/bin/git", "git --help", ".."] {
            assert!(!valid_command(value), "{value}");
        }
        let validated =
            validate_commands(&commands, &["git".to_string(), "Git".to_string()]).unwrap();
        assert_eq!(validated.into_iter().collect::<Vec<_>>(), vec!["git"]);
        assert!(validate_commands(&BTreeSet::new(), &["git".into()]).is_err());
        let grant = PluginGrant {
            approved: true,
            enabled: true,
            granted: commands.clone(),
            network_domains: BTreeSet::new(),
            commands: ["git".to_string()].into(),
        };
        assert_eq!(
            effective(Some(&grant), &commands)
                .commands
                .into_iter()
                .collect::<Vec<_>>(),
            vec!["git"]
        );
    }

    #[test]
    fn domain_scopes_are_exact_and_require_explicit_grants() {
        let declared = declared(&["network"]);
        assert_eq!(normalize_domain("Example.COM").unwrap(), "example.com");
        for value in [
            "*.example.com",
            "https://example.com",
            "example.com:443",
            ".example.com",
            "evil/example.com",
            "",
        ] {
            assert!(normalize_domain(value).is_err(), "{value}");
        }
        assert!(validate_network_domains(&BTreeSet::new(), &["example.com".into()]).is_err());
        let grant = effective(
            Some(&PluginGrant {
                approved: true,
                enabled: true,
                granted: declared.clone(),
                network_domains: ["example.com".to_string()].into(),
                commands: BTreeSet::new(),
            }),
            &declared,
        );
        assert!(grant.network_domains.contains("example.com"));
        assert!(!grant.network_domains.contains("sub.example.com"));
        assert_eq!(normalize_domain("[::1]").unwrap(), "::1");
        assert!(is_loopback_domain("localhost"));
        assert!(is_loopback_domain("127.0.0.1"));
        assert!(is_loopback_domain("::1"));
        assert!(!is_loopback_domain("192.168.1.2"));
    }
}
