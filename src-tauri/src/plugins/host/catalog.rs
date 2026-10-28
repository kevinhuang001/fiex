//! Installed-plugin discovery and catalog assembly.
use super::manifest::{activation_requirements, declared, valid_id, PluginAuthor, PluginManifest};
use crate::plugin_sdk;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
#[cfg(not(test))]
use std::sync::{Mutex, OnceLock};
#[cfg(not(test))]
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub icon: Option<String>,
    pub extensions: Value,
    /// Explicit `extensions.ui.timeoutMs` budget (already bounded), used by the
    /// panel bridge and projected to the plugin UI for its own timers.
    pub ui_timeout_ms: u64,
    pub backend_available: bool,
    pub enabled: bool,
    pub author: Option<PluginAuthor>,
    pub approved: bool,
    pub declared_permissions: Vec<String>,
    pub granted_permissions: Vec<String>,
    pub requested_network_all: bool,
    pub requested_network_domains: Vec<String>,
    pub allowed_network_all: bool,
    pub allowed_network_domains: Vec<String>,
    pub requested_commands: Vec<String>,
    pub allowed_commands: Vec<String>,
    /// Requested commands that cannot be resolved on PATH right now. The
    /// settings UI marks these red: the plugin is approved to run them but the
    /// machine cannot.
    pub missing_commands: Vec<String>,
}

fn roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    // The user-level plugin installation directory is the sole release source.
    if let Some(data) = dirs::data_local_dir() {
        roots.push(data.join("fiex").join("plugins"));
    }
    // Test isolation override is unavailable in packaged release builds.
    #[cfg(debug_assertions)]
    if let Some(path) = std::env::var_os("FIEX_PLUGIN_DIR") {
        roots.insert(0, PathBuf::from(path));
    }
    roots
}
pub(crate) fn discover() -> Vec<(PluginManifest, PathBuf)> {
    let mut by_id = HashMap::new();
    for root in roots() {
        let candidates = std::iter::once(root.clone()).chain(
            fs::read_dir(&root)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path()),
        );
        for folder in candidates {
            let manifest_path = folder.join("manifest.json");
            let Ok(bytes) = fs::read(&manifest_path) else {
                continue;
            };
            if bytes.len() > 1024 * 1024 {
                continue;
            }
            let Ok(manifest) = serde_json::from_slice::<PluginManifest>(&bytes) else {
                continue;
            };
            if let Err(error) = super::manifest::validate_extensions(&manifest.extensions) {
                eprintln!("plugin {} manifest rejected: {error}", manifest.id);
                continue;
            }
            if manifest.schema_version != 2
                || (!crate::plugins::themes::is_theme(&manifest)
                    && manifest.runtime.protocol_version != plugin_sdk::PROTOCOL_VERSION)
                || !valid_id(&manifest.id)
                || manifest.name.len() > 128
                || declared(&manifest).is_err()
            {
                continue;
            }
            by_id
                .entry(manifest.id.clone())
                .or_insert((manifest, folder));
        }
    }
    let mut list = by_id.into_values().collect::<Vec<_>>();
    list.sort_by(|a, b| a.0.name.cmp(&b.0.name));
    list
}

/// Discovery is used by every provider match. Cache only the manifest scan;
/// permission state is read on every catalog request and invocation still
/// performs fresh discovery and authorization.
#[cfg(not(test))]
type DiscoverySnapshot = Mutex<Option<(Instant, Vec<(PluginManifest, PathBuf)>)>>;
#[cfg(not(test))]
static DISCOVERY: OnceLock<DiscoverySnapshot> = OnceLock::new();

pub(crate) fn invalidate_discovery() {
    #[cfg(not(test))]
    if let Some(cache) = DISCOVERY.get() {
        if let Ok(mut snapshot) = cache.lock() {
            *snapshot = None;
        }
    }
}

fn discovery_for_matching() -> Vec<(PluginManifest, PathBuf)> {
    #[cfg(test)]
    {
        discover()
    }
    #[cfg(not(test))]
    {
        let snapshot = DISCOVERY.get_or_init(|| Mutex::new(None));
        if let Ok(guard) = snapshot.lock() {
            if let Some((at, manifests)) = guard.as_ref() {
                if at.elapsed() < Duration::from_secs(2) {
                    return manifests.clone();
                }
            }
        }
        let fresh = discover();
        if let Ok(mut guard) = snapshot.lock() {
            *guard = Some((Instant::now(), fresh.clone()));
        }
        fresh
    }
}
fn platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}
/// Commands Core would fail to spawn because they are not on PATH today.
fn missing_commands(commands: &std::collections::BTreeSet<String>) -> Vec<String> {
    commands
        .iter()
        .filter(|command| which::which(command).is_err())
        .cloned()
        .collect()
}
pub(crate) fn canonical_under(candidate: &Path, root: &Path) -> bool {
    match (fs::canonicalize(candidate), fs::canonicalize(root)) {
        (Ok(candidate), Ok(root)) => candidate.starts_with(&root) && candidate != root,
        _ => false,
    }
}
pub(crate) fn executable(manifest: &PluginManifest, dir: &Path) -> Result<PathBuf, String> {
    let key = platform();
    let rel = manifest
        .runtime
        .entrypoints
        .get(&key)
        .ok_or("plugin has no native backend for platform")?;
    let path = Path::new(rel);
    if !path.components().all(|c| matches!(c, Component::Normal(_))) {
        return Err("invalid plugin executable path".into());
    }
    let inside = dir.join(path);
    if inside.is_file() && canonical_under(&inside, dir) {
        return Ok(inside);
    }
    // An executable outside the declared plugin package is never allowed.
    Err("plugin native backend is not installed inside its package".into())
}
pub fn catalog() -> Vec<PluginInfo> {
    catalog_matching(|_| true)
}

/// Apply event match predicates before permission, executable and PATH probes.
pub(crate) fn catalog_matching(predicate: impl Fn(&PluginManifest) -> bool) -> Vec<PluginInfo> {
    discovery_for_matching()
        .into_iter()
        .filter(|(manifest, _)| predicate(manifest))
        .map(|(manifest, folder)| {
            let manifest = super::localize::manifest(manifest);
            let backend = crate::plugins::themes::is_theme(&manifest)
                || executable(&manifest, &folder).is_ok();
            let declared = declared(&manifest).unwrap_or_default();
            let state = crate::plugins::security::permissions::status(&manifest.id, &declared)
                .unwrap_or_default(); // unreadable policy fails closed
                                      // `enabled` means the package may run. Individual capabilities may
                                      // be switched off; their operations ask again instead of
                                      // deactivating the whole plugin.
            let approved = state.approved && declared.is_subset(&state.approved_declared);
            let active = approved && state.enabled;
            let requested = activation_requirements(&manifest).unwrap_or_default();
            let missing_commands = missing_commands(&requested.commands);
            let ui_timeout_ms = super::manifest::ui_timeout_ms(&manifest.extensions);
            PluginInfo {
                schema_version: manifest.schema_version,
                id: manifest.id,
                name: manifest.name,
                version: manifest.version,
                description: manifest.description,
                icon: manifest.icon,
                extensions: manifest.extensions,
                ui_timeout_ms,
                backend_available: backend,
                enabled: active,
                author: manifest.author,
                approved,
                declared_permissions: declared.into_iter().collect(),
                granted_permissions: state.granted.into_iter().collect(),
                requested_network_all: requested.network_all,
                requested_network_domains: requested.domains.into_iter().collect(),
                allowed_network_all: state.network_all,
                allowed_network_domains: state.network_domains.into_iter().collect(),
                requested_commands: requested.commands.into_iter().collect(),
                allowed_commands: state.commands.into_iter().collect(),
                missing_commands,
            }
        })
        .collect()
}
pub fn installed_manifests() -> Vec<(PluginManifest, PathBuf)> {
    discover()
}
pub fn exists(id: &str) -> bool {
    discover().iter().any(|(m, _)| m.id == id)
}
pub fn themes() -> Vec<crate::plugins::themes::Theme> {
    discover()
        .into_iter()
        .filter(|(manifest, _)| {
            activation_requirements(manifest).is_ok_and(|required| {
                crate::plugins::security::permissions::status(&manifest.id, &required.permissions)
                    .is_ok_and(|state| state.approved && state.enabled)
            })
        })
        .flat_map(|(manifest, folder)| {
            crate::plugins::themes::load(&manifest, &folder).unwrap_or_default()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_commands_only_reports_unresolvable_names() {
        let commands = std::collections::BTreeSet::from([
            "fiex-definitely-not-a-real-command".to_string(),
            "git".to_string(),
        ]);
        let missing = missing_commands(&commands);
        assert_eq!(
            missing,
            vec!["fiex-definitely-not-a-real-command".to_string()]
        );
        assert!(missing_commands(&std::collections::BTreeSet::new()).is_empty());
    }
}
