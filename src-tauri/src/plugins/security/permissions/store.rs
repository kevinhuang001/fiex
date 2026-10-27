use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

use super::model::PluginGrant;
use super::targets::{normalize_domain, valid_command};

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

pub(super) fn effective(record: Option<&PluginGrant>, declared: &BTreeSet<String>) -> PluginGrant {
    match record {
        Some(r) if r.approved => PluginGrant {
            approved: true,
            enabled: r.enabled,
            granted: r.granted.intersection(declared).cloned().collect(),
            approved_declared: r
                .approved_declared
                .intersection(declared)
                .cloned()
                .collect(),
            network_domains: if declared.contains("network") {
                r.network_domains
                    .iter()
                    .filter(|d| normalize_domain(d).is_ok())
                    .cloned()
                    .collect()
            } else {
                BTreeSet::new()
            },
            network_all: declared.contains("network") && r.network_all,
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

pub(super) fn clear(id: &str) -> Result<(), String> {
    let _guard = gate()
        .lock()
        .map_err(|_| "permission store lock poisoned")?;
    let mut state = load()?;
    state.plugins.insert(id.to_owned(), PluginGrant::default());
    save(&state)
}

pub(super) fn mutate(
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
