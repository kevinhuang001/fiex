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
pub fn approve_install(id: &str, declared: &BTreeSet<String>) -> Result<PluginGrant, String> {
    mutate(id, declared, |current| {
        if current.approved {
            return Err("plugin is already authorized".into());
        }
        current.approved = true;
        current.enabled = true;
        current.granted = declared.clone();
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
        };
        let new = declaration(&serde_json::json!({"storage":true,"secrets":true})).unwrap();
        let s = effective(Some(&current), &new);
        assert!(s.granted.contains("storage") && !s.granted.contains("secrets"));
    }
    #[test]
    fn invalid_permissions_fail_closed() {
        assert!(declaration(&serde_json::json!({"root":true})).is_err());
        assert!(declaration(&serde_json::json!({"storage":"yes"})).is_err());
    }
}
