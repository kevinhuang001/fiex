//! All installed plugins use identical host-authorized permissions.
//! The publisher name, GitHub repository, and package location confer no
//! privileges. Installation requires explicit approval of declared capabilities.
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

pub const PERMISSIONS: &[&str] = &[
    "storage",
    "secrets",
    "network",
    "filesystem.read",
    "filesystem.write",
    "externalCommands",
    "ui.dialog",
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
pub fn declaration(raw: &serde_json::Value) -> Result<BTreeSet<String>, String> {
    let object = raw
        .as_object()
        .ok_or("manifest.permissions must be an object")?;
    let mut declared = BTreeSet::new();
    for (name, value) in object {
        if !PERMISSIONS.contains(&name.as_str()) {
            return Err(format!("unknown permission: {name}"));
        }
        if !value.is_boolean() {
            return Err(format!("permission must be boolean: {name}"));
        }
        if value == true {
            declared.insert(name.clone());
        }
    }
    Ok(declared)
}
fn effective(record: Option<&PluginGrant>, declared: &BTreeSet<String>) -> PluginGrant {
    match record {
        Some(r) if r.approved => PluginGrant {
            approved: true,
            enabled: r.enabled,
            granted: r.granted.intersection(declared).cloned().collect(),
            network_domains: if declared.contains("network") {
                r.network_domains.iter().filter(|d| normalize_domain(d).is_ok()).cloned().collect()
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

/// Normalize an exact DNS hostname or IPv4 address. Patterns, schemes, URLs,
/// ports and wildcard suffixes are not permission scopes.
pub fn normalize_domain(input: &str) -> Result<String, String> {
    let domain = input.trim().to_ascii_lowercase();
    if domain.is_empty() || domain.len() > 253 || !domain.is_ascii() {
        return Err("domain must be a nonempty ASCII hostname (use IDNA/punycode)".into());
    }
    if domain.split('.').any(|label| {
        label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    }) {
        return Err("invalid hostname: enter an exact domain without a wildcard or port".into());
    }
    Ok(domain)
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
/// for future broker operations; the plugin launcher is restarted by the host.
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
        }
        Ok(())
    })
}

pub fn domain_allowed(
    id: &str,
    declared: &BTreeSet<String>,
    domain: &str,
) -> Result<bool, String> {
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
    approve_install_with_domains(id, declared, &[])
}

/// The installation consent explicitly covers the requested exact domains.
pub fn approve_install_with_domains(
    id: &str,
    declared: &BTreeSet<String>,
    domains: &[String],
) -> Result<PluginGrant, String> {
    let domains = validate_network_domains(declared, domains)?;
    mutate(id, declared, |current| {
        if current.approved {
            return Err("plugin is already authorized".into());
        }
        current.approved = true;
        current.enabled = true;
        current.granted = declared.clone();
        current.network_domains = domains.clone();
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
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_plugins_start_unapproved() {
        let requested = declaration(&serde_json::json!({"storage":true,"secrets":true})).unwrap();
        let state = effective(None, &requested);
        assert!(!state.approved && !state.enabled && state.granted.is_empty());
    }
    #[test]
    fn removed_grants_stay_removed_and_new_scopes_never_auto_grant() {
        let old = declaration(&serde_json::json!({"storage":true})).unwrap();
        let current = PluginGrant {
            approved: true,
            enabled: true,
            granted: old,
            network_domains: BTreeSet::new(),
        };
        let new = declaration(&serde_json::json!({"storage":true,"secrets":true})).unwrap();
        let s = effective(Some(&current), &new);
        assert!(s.granted.contains("storage") && !s.granted.contains("secrets"));
    }
    #[test]
    fn domain_scopes_are_exact_and_require_explicit_grants() {
        let declared = declaration(&serde_json::json!({"network": true})).unwrap();
        assert_eq!(normalize_domain("Example.COM").unwrap(), "example.com");
        for value in ["*.example.com", "https://example.com", "example.com:443", ".example.com", "evil/example.com", ""] {
            assert!(normalize_domain(value).is_err(), "{value}");
        }
        assert!(validate_network_domains(&BTreeSet::new(), &["example.com".into()]).is_err());
        let grant = effective(
            Some(&PluginGrant {
                approved: true,
                enabled: true,
                granted: declared.clone(),
                network_domains: ["example.com".to_string()].into(),
            }),
            &declared,
        );
        assert!(grant.network_domains.contains("example.com"));
        assert!(!grant.network_domains.contains("sub.example.com"));
    }
    #[test]
    fn invalid_permissions_fail_closed() {
        assert!(declaration(&serde_json::json!({"root":true})).is_err());
        assert!(declaration(&serde_json::json!({"storage":"yes"})).is_err());
    }
}
