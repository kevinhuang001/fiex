//! Manifest-driven installed-plugin discovery. Every plugin must be approved
//! by the user; no ID, publisher name or package location grants privileges.
use crate::plugin_sdk;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs,
    io::BufReader,
    path::{Component, Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{Arc, Mutex, OnceLock},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub entrypoints: HashMap<String, String>,
    #[serde(default)]
    pub contributes: Value,
    #[serde(default)]
    pub permissions: Value,
    #[serde(default)]
    pub author: Option<PluginAuthor>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginAuthor {
    pub name: String,
    #[serde(default)]
    pub url: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub contributes: Value,
    pub backend_available: bool,
    pub enabled: bool,
    pub author: Option<PluginAuthor>,
    pub approved: bool,
    pub declared_permissions: Vec<String>,
    pub granted_permissions: Vec<String>,
}
fn declared(manifest: &PluginManifest) -> Result<std::collections::BTreeSet<String>, String> {
    crate::plugin_permissions::declaration(&manifest.permissions)
}
pub fn set_enabled(id: &str, enabled: bool) -> Result<(), String> {
    let (manifest, _) = discover()
        .into_iter()
        .find(|(m, _)| m.id == id)
        .ok_or("plugin not installed")?;
    crate::plugin_permissions::set_enabled(id, &declared(&manifest)?, enabled)?;
    global().stop(id);
    Ok(())
}
pub fn permission_allowed(id: &str, permission: &str) -> Result<bool, String> {
    let (manifest, _) = discover()
        .into_iter()
        .find(|(m, _)| m.id == id)
        .ok_or("plugin not installed")?;
    crate::plugin_permissions::allowed(id, &declared(&manifest)?, permission)
}
pub fn set_permission(id: &str, permission: &str, grant: bool) -> Result<(), String> {
    let (manifest, _) = discover()
        .into_iter()
        .find(|(m, _)| m.id == id)
        .ok_or("plugin not installed")?;
    crate::plugin_permissions::set_granted(id, &declared(&manifest)?, permission, grant)?;
    global().stop(id);
    Ok(())
}
pub fn approve_install(id: &str) -> Result<(), String> {
    let (manifest, _) = discover()
        .into_iter()
        .find(|(m, _)| m.id == id)
        .ok_or("plugin not installed")?;
    crate::plugin_permissions::approve_install(id, &declared(&manifest)?)?;
    Ok(())
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
fn discover() -> Vec<(PluginManifest, PathBuf)> {
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
            if !valid_id(&manifest.id) || manifest.name.len() > 128 || declared(&manifest).is_err()
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
fn platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}
pub fn valid_id(id: &str) -> bool {
    id.len() > 2
        && id.len() < 128
        && id.contains('.')
        && !id.starts_with('.')
        && !id.contains("..")
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}
fn canonical_under(candidate: &Path, root: &Path) -> bool {
    match (fs::canonicalize(candidate), fs::canonicalize(root)) {
        (Ok(candidate), Ok(root)) => candidate.starts_with(&root) && candidate != root,
        _ => false,
    }
}
fn executable(manifest: &PluginManifest, dir: &Path) -> Result<PathBuf, String> {
    let key = platform();
    let rel = manifest
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
    discover()
        .into_iter()
        .map(|(manifest, folder)| {
            let backend = executable(&manifest, &folder).is_ok();
            let declared = declared(&manifest).unwrap_or_default();
            let state =
                crate::plugin_permissions::status(&manifest.id, &declared).unwrap_or_default(); // unreadable policy fails closed
            PluginInfo {
                id: manifest.id,
                name: manifest.name,
                version: manifest.version,
                capabilities: manifest.capabilities,
                contributes: manifest.contributes,
                backend_available: backend,
                enabled: state.enabled && state.approved,
                author: manifest.author,
                approved: state.approved,
                declared_permissions: declared.into_iter().collect(),
                granted_permissions: state.granted.into_iter().collect(),
            }
        })
        .collect()
}
/// Host-approved precompiled, self-contained iframe HTML. Plugins cannot
/// supply absolute paths or read unrelated files through this API.
pub fn panel_html(plugin_id: &str, panel_id: &str) -> Result<String, String> {
    let (manifest, folder) = discover()
        .into_iter()
        .find(|(m, _)| m.id == plugin_id)
        .ok_or("plugin is not installed")?;
    let rights = crate::plugin_permissions::status(plugin_id, &declared(&manifest)?)?;
    if !rights.approved || !rights.enabled {
        return Err("plugin disabled or not approved".into());
    }
    let panels = manifest
        .contributes
        .get("panels")
        .and_then(Value::as_array)
        .ok_or("plugin declares no panels")?;
    let panel = panels
        .iter()
        .find(|p| p.get("id").and_then(Value::as_str) == Some(panel_id))
        .ok_or("unknown panel id")?;
    let asset = panel
        .get("entry")
        .and_then(Value::as_str)
        .ok_or("panel entry missing")?;
    let rel = Path::new(asset);
    if !rel.components().all(|c| matches!(c, Component::Normal(_))) {
        return Err("invalid panel resource path".into());
    }
    let file = folder.join(rel);
    if !canonical_under(&file, &folder) {
        return Err("panel resource outside plugin package".into());
    }
    let info = fs::metadata(&file).map_err(|e| e.to_string())?;
    if !info.is_file() || info.len() > 1024 * 1024 {
        return Err("panel HTML exceeds size limit".into());
    }
    fs::read_to_string(file).map_err(|e| e.to_string())
}
pub fn exists(id: &str) -> bool {
    discover().iter().any(|(m, _)| m.id == id)
}
struct Running {
    plugin_id: String,
    declared: std::collections::BTreeSet<String>,
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}
impl Running {
    /// All host.* requests are authenticated by the Running process identity,
    /// never by a pluginId supplied in the message.
    fn host_method(&self, method: &str, p: &Value) -> Result<Value, String> {
        let permitted = |name: &str| {
            crate::plugin_permissions::allowed(&self.plugin_id, &self.declared, name)
                .unwrap_or(false)
        };
        let arg = |key: &str| {
            p.get(key)
                .and_then(Value::as_str)
                .ok_or_else(|| format!("missing {key}"))
        };
        let scope = || arg("scope");
        let key = || arg("key");
        match method {
            "host.storage.get" if permitted("storage") => {
                Ok(
                    crate::plugin_storage::get(&self.plugin_id, scope()?, key()?)?
                        .unwrap_or(Value::Null),
                )
            }
            "host.storage.set" if permitted("storage") => {
                let value = p.get("value").cloned().ok_or("missing value")?;
                crate::plugin_storage::set(&self.plugin_id, scope()?, key()?, value)?;
                Ok(Value::Null)
            }
            "host.storage.delete" if permitted("storage") => {
                crate::plugin_storage::delete(&self.plugin_id, scope()?, key()?)?;
                Ok(Value::Null)
            }
            "host.storage.keys" if permitted("storage") => Ok(json!(
                crate::plugin_storage::list_keys(&self.plugin_id, scope()?)?
            )),
            "host.secret.get" if permitted("secrets") => Ok(json!(
                crate::plugin_storage::secret_get(&self.plugin_id, scope()?, key()?)?
            )),
            "host.secret.set" if permitted("secrets") => {
                crate::plugin_storage::secret_set(
                    &self.plugin_id,
                    scope()?,
                    key()?,
                    arg("value")?,
                )?;
                Ok(Value::Null)
            }
            "host.secret.delete" if permitted("secrets") => {
                crate::plugin_storage::secret_delete(&self.plugin_id, scope()?, key()?)?;
                Ok(Value::Null)
            }
            "host.secret.available" if permitted("secrets") => {
                Ok(json!(crate::plugin_storage::secret_available()))
            }
            _ => Err("host method not available or permission denied".into()),
        }
    }
    fn rpc(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.next_id += 1;
        let id = self.next_id;
        plugin_sdk::write_frame(
            &mut self.stdin,
            &json!({
                "jsonrpc":"2.0","id":id,"method":method,"params":params
            }),
        )
        .map_err(|e| e.to_string())?;
        loop {
            let response = plugin_sdk::read_frame(&mut self.stdout)
                .map_err(|e| e.to_string())?
                .ok_or("plugin exited during request")?;
            // Re-entrant plugin→host request while host awaits a plugin response.
            if let Some(host_method) = response.get("method").and_then(Value::as_str) {
                let Some(reverse_id) = response.get("id").cloned() else {
                    continue;
                };
                let parameters = response.get("params").cloned().unwrap_or(Value::Null);
                let reply = match self.host_method(host_method, &parameters) {
                    Ok(value) => json!({"jsonrpc":"2.0","id":reverse_id,"result":value}),
                    Err(message) => json!({"jsonrpc":"2.0","id":reverse_id,
                        "error":{"code":-32001,"message":message}}),
                };
                plugin_sdk::write_frame(&mut self.stdin, &reply).map_err(|e| e.to_string())?;
                continue;
            }
            if response.get("id").and_then(Value::as_u64) != Some(id) {
                return Err("plugin response ID mismatch".into());
            }
            if let Some(error) = response.get("error") {
                return Err(format!("plugin error: {error}"));
            }
            return response
                .get("result")
                .cloned()
                .ok_or_else(|| "plugin response missing result".into());
        }
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
#[derive(Default)]
pub struct PluginHost {
    children: Mutex<HashMap<String, Arc<Mutex<Running>>>>,
}
impl PluginHost {
    pub fn stop(&self, id: &str) {
        if let Ok(mut processes) = self.children.lock() {
            // Removes idle processes. A request currently holding an Arc
            // is still subject to per-IPC checks; strong immediate termination
            // requires the platform sandbox/kill-handle launcher.
            processes.remove(id);
        }
    }
    pub fn is_installed(&self, id: &str) -> bool {
        discover()
            .into_iter()
            .find(|(m, _)| m.id == id)
            .is_some_and(|(m, d)| executable(&m, &d).is_ok())
    }
    pub fn invoke(&self, plugin_id: &str, method: &str, params: Value) -> Result<Value, String> {
        let mut guard = self
            .children
            .lock()
            .map_err(|_| "plugin host lock poisoned")?;
        let instance = if let Some(instance) = guard.get(plugin_id) {
            instance.clone()
        } else {
            let (manifest, dir) = discover()
                .into_iter()
                .find(|(m, _)| m.id == plugin_id)
                .ok_or("plugin is not installed")?;
            let declared = declared(&manifest)?;
            let state = crate::plugin_permissions::status(plugin_id, &declared)?;
            if !state.enabled || !state.approved {
                return Err("plugin is disabled or installation consent is pending".into());
            }
            let path = executable(&manifest, &dir)?;
            let mut child = Command::new(&path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .map_err(|e| format!("plugin start failed: {e}"))?;
            let stdin = child.stdin.take().ok_or("plugin stdin unavailable")?;
            let stdout = child.stdout.take().ok_or("plugin stdout unavailable")?;
            let mut running = Running {
                plugin_id: plugin_id.to_string(),
                declared,
                child,
                stdin,
                stdout: BufReader::new(stdout),
                next_id: 0,
            };
            let result = running.rpc(
                "plugin.initialize",
                json!({"protocolVersion":plugin_sdk::PROTOCOL_VERSION}),
            )?;
            if result.get("pluginId").and_then(Value::as_str) != Some(plugin_id) {
                return Err("plugin identity mismatch".into());
            }
            let instance = Arc::new(Mutex::new(running));
            guard.insert(plugin_id.to_string(), instance.clone());
            instance
        };
        drop(guard);
        let response = instance
            .lock()
            .map_err(|_| "plugin instance lock poisoned")?
            .rpc(method, params);
        if response.is_err() {
            if let Ok(mut guard) = self.children.lock() {
                guard.remove(plugin_id);
            }
        }
        response
    }
}
pub fn global() -> &'static PluginHost {
    static HOST: OnceLock<PluginHost> = OnceLock::new();
    HOST.get_or_init(PluginHost::default)
}
