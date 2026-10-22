//! Host-owned plugin activation and permission decisions.
//!
//! Bundled plugins start enabled with all permissions declared by their
//! verified manifests. Revocations persist. Third-party plugins require an
//! explicit install-time approval; they never receive bundled privileges just
//! because they copy an official ID. All data is default-deny on error.
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
    /// Explicit denials of builtin permissions survive updates/restarts.
    #[serde(default)]
    pub denied: BTreeSet<String>,
}

#[derive(Default, Serialize, Deserialize)]
struct PolicyStore {
    #[serde(default)]
    plugins: BTreeMap<String, PluginGrant>,
}

fn lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}
fn path() -> Result<PathBuf, String> {
    dirs::data_local_dir()
        .map(|p| p.join("fiex").join("plugin-permissions.json"))
        .ok_or_else(|| "application data directory unavailable".into())
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
fn save(store: &PolicyStore) -> Result<(), String> {
    let destination = path()?;
    let parent = destination
        .parent()
        .ok_or("permission path missing parent")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let buffer = serde_json::to_vec_pretty(store).map_err(|e| e.to_string())?;
    let temp = parent.join(format!("plugin-permissions-{}.tmp", std::process::id()));
    let result = (|| {
        let mut out = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        out.write_all(&buffer).map_err(|e| e.to_string())?;
        out.sync_all().map_err(|e| e.to_string())?;
        // On Windows atomic replacement is implemented with platform-specific
        // APIs in the final storage backend; fail closed if rename is denied.
        fs::rename(&temp, &destination).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}
pub fn declaration(raw: &serde_json::Value) -> Result<BTreeSet<String>, String> {
    let obj = raw
        .as_object()
        .ok_or("manifest.permissions must be an object")?;
    let mut set = BTreeSet::new();
    for (permission, value) in obj {
        if !PERMISSIONS.contains(&permission.as_str()) {
            return Err(format!("unknown permission: {permission}"));
        }
        if !value.is_boolean() {
            return Err(format!("invalid permission declaration: {permission}"));
        }
        if value == true {
            set.insert(permission.clone());
        }
    }
    Ok(set)
}
fn effective(
    record: Option<&PluginGrant>,
    declared: &BTreeSet<String>,
    bundled: bool,
) -> PluginGrant {
    if bundled {
        match record {
            Some(current) => PluginGrant {
                approved: true,
                enabled: current.enabled,
                granted: declared.difference(&current.denied).cloned().collect(),
                denied: current.denied.intersection(declared).cloned().collect(),
            },
            None => PluginGrant {
                approved: true,
                enabled: true,
                granted: declared.clone(),
                denied: BTreeSet::new(),
            },
        }
    } else {
        match record {
            Some(current) if current.approved => PluginGrant {
                approved: true,
                enabled: current.enabled,
                granted: current.granted.intersection(declared).cloned().collect(),
                denied: current.denied.intersection(declared).cloned().collect(),
            },
            _ => PluginGrant::default(),
        }
    }
}
pub fn status(id: &str, declared: &BTreeSet<String>, bundled: bool) -> Result<PluginGrant, String> {
    let _guard = lock().lock().map_err(|_| "permission mutex poisoned")?;
    let store = load()?;
    Ok(effective(store.plugins.get(id), declared, bundled))
}
pub fn allowed(
    id: &str,
    declared: &BTreeSet<String>,
    bundled: bool,
    permission: &str,
) -> Result<bool, String> {
    if !declared.contains(permission) {
        return Ok(false);
    }
    let state = status(id, declared, bundled)?;
    Ok(state.approved && state.enabled && state.granted.contains(permission))
}
pub fn approve_install(
    id: &str,
    declared: &BTreeSet<String>,
    bundled: bool,
) -> Result<PluginGrant, String> {
    if bundled {
        return status(id, declared, true);
    }
    mutate(id, declared, false, |record| {
        if record.approved {
            return Err("plugin already approved".into());
        }
        record.approved = true;
        record.enabled = true;
        record.granted = declared.clone();
        Ok(())
    })
}
pub fn set_enabled(
    id: &str,
    declared: &BTreeSet<String>,
    bundled: bool,
    enabled: bool,
) -> Result<PluginGrant, String> {
    mutate(id, declared, bundled, |record| {
        if !record.approved {
            return Err("install approval required".into());
        }
        record.enabled = enabled;
        Ok(())
    })
}
pub fn set_granted(
    id: &str,
    declared: &BTreeSet<String>,
    bundled: bool,
    permission: &str,
    granted: bool,
) -> Result<PluginGrant, String> {
    if !declared.contains(permission) {
        return Err("undeclared permission".into());
    }
    mutate(id, declared, bundled, |record| {
        if !record.approved {
            return Err("install approval required".into());
        }
        if granted {
            record.denied.remove(permission);
            record.granted.insert(permission.into());
        } else {
            record.denied.insert(permission.into());
            record.granted.remove(permission);
        }
        Ok(())
    })
}
fn mutate(
    id: &str,
    declared: &BTreeSet<String>,
    bundled: bool,
    change: impl FnOnce(&mut PluginGrant) -> Result<(), String>,
) -> Result<PluginGrant, String> {
    let _guard = lock().lock().map_err(|_| "permission mutex poisoned")?;
    let mut store = load()?;
    let mut next = effective(store.plugins.get(id), declared, bundled);
    change(&mut next)?;
    store.plugins.insert(id.to_string(), next.clone());
    save(&store)?;
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn official_is_granted_by_default_but_revocation_sticks() {
        let declared = declaration(&serde_json::json!({"secrets":true,"storage":true})).unwrap();
        let initial = effective(None, &declared, true);
        assert!(initial.enabled && initial.granted.contains("secrets"));
        let mut revoked = initial.clone();
        revoked.denied.insert("secrets".into());
        let next = effective(Some(&revoked), &declared, true);
        assert!(!next.granted.contains("secrets"));
        assert!(next.granted.contains("storage"));
    }
    #[test]
    fn untrusted_identity_does_not_inherit_builtin_grants() {
        let declared = declaration(&serde_json::json!({"secrets":true})).unwrap();
        let state = effective(None, &declared, false);
        assert!(!state.approved && !state.enabled && state.granted.is_empty());
    }
    #[test]
    fn extra_or_invalid_manifest_permissions_are_rejected() {
        assert!(declaration(&serde_json::json!({"root":true})).is_err());
        assert!(declaration(&serde_json::json!({"secrets":"true"})).is_err());
    }
}
