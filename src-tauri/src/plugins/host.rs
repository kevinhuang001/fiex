//! Manifest-driven native plugin discovery. Official and third-party packages
//! share the same loader. Plugin IDs and paths never come from untrusted RPC.
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
    pub official: bool,
    pub approved: bool,
    pub declared_permissions: Vec<String>,
    pub granted_permissions: Vec<String>,
}
fn valid_id(s: &str) -> bool {
    s.len() > 2
        && s.len() < 128
        && !s.starts_with('.')
        && s.contains('.')
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}
static BUNDLED_ROOT: OnceLock<PathBuf> = OnceLock::new();
const OFFICIAL_IDS: &[&str] = &[
    "org.fiex.git",
    "org.fiex.webdav",
    "org.fiex.smb",
    "org.fiex.archive",
    "org.fiex.checksum",
];
fn builtin_manifest(id: &str) -> Option<&'static str> {
    match id {
        "org.fiex.git" => Some(include_str!("../../../plugins/official/git/manifest.json")),
        "org.fiex.webdav" => Some(include_str!(
            "../../../plugins/official/webdav/manifest.json"
        )),
        "org.fiex.smb" => Some(include_str!("../../../plugins/official/smb/manifest.json")),
        "org.fiex.archive" => Some(include_str!(
            "../../../plugins/official/archive/manifest.json"
        )),
        "org.fiex.checksum" => Some(include_str!(
            "../../../plugins/official/checksum/manifest.json"
        )),
        _ => None,
    }
}
fn canonical_under(folder: &Path, root: &Path) -> bool {
    match (fs::canonicalize(folder), fs::canonicalize(root)) {
        (Ok(folder), Ok(root)) => folder.starts_with(root) && folder != root,
        _ => false,
    }
}
fn trusted_builtin(manifest: &PluginManifest, folder: &Path) -> bool {
    let Some(expected) = builtin_manifest(&manifest.id) else {
        return false;
    };
    let packaged = BUNDLED_ROOT
        .get()
        .is_some_and(|root| canonical_under(folder, root));
    // A debug build's staged plugin directory is trusted only for development.
    let debug_staged = cfg!(debug_assertions)
        && canonical_under(
            folder,
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins"),
        );
    if !packaged && !debug_staged {
        return false;
    }
    let Ok(bytes) = fs::read(folder.join("manifest.json")) else {
        return false;
    };
    let Ok(found) = serde_json::from_slice::<Value>(&bytes) else {
        return false;
    };
    let Ok(reference) = serde_json::from_str::<Value>(expected) else {
        return false;
    };
    found == reference
}
fn declared(manifest: &PluginManifest) -> Result<std::collections::BTreeSet<String>, String> {
    crate::plugin_permissions::declaration(&manifest.permissions)
}
pub fn permission_status(
    id: &str,
) -> Result<(bool, crate::plugin_permissions::PluginGrant, Vec<String>), String> {
    let (manifest, folder) = discover()
        .into_iter()
        .find(|(m, _)| m.id == id)
        .ok_or("plugin not installed")?;
    let declared = declared(&manifest)?;
    let bundled = trusted_builtin(&manifest, &folder);
    let status = crate::plugin_permissions::status(id, &declared, bundled)?;
    Ok((bundled, status, declared.into_iter().collect()))
}
pub fn set_enabled(id: &str, enabled: bool) -> Result<(), String> {
    let (manifest, folder) = discover()
        .into_iter()
        .find(|(m, _)| m.id == id)
        .ok_or("plugin not installed")?;
    let declared = declared(&manifest)?;
    crate::plugin_permissions::set_enabled(
        id,
        &declared,
        trusted_builtin(&manifest, &folder),
        enabled,
    )?;
    global().stop(id);
    Ok(())
}
pub fn set_permission(id: &str, permission: &str, grant: bool) -> Result<(), String> {
    let (manifest, folder) = discover()
        .into_iter()
        .find(|(m, _)| m.id == id)
        .ok_or("plugin not installed")?;
    let declared = declared(&manifest)?;
    crate::plugin_permissions::set_granted(
        id,
        &declared,
        trusted_builtin(&manifest, &folder),
        permission,
        grant,
    )?;
    global().stop(id);
    Ok(())
}
pub fn approve_install(id: &str) -> Result<(), String> {
    let (manifest, folder) = discover()
        .into_iter()
        .find(|(m, _)| m.id == id)
        .ok_or("plugin not installed")?;
    let declared = declared(&manifest)?;
    crate::plugin_permissions::approve_install(id, &declared, trusted_builtin(&manifest, &folder))?;
    Ok(())
}

pub fn set_bundle_root(resource_root: PathBuf) {
    let _ = BUNDLED_ROOT.set(resource_root.join("plugins"));
}
fn roots() -> Vec<PathBuf> {
    let mut all = Vec::new();
    // App-owned bundle comes first; an environment override is NEVER proof
    // that a plugin is official.
    // Bundled, verified first-party IDs take precedence over user directories.
    if let Some(bundled) = BUNDLED_ROOT.get() {
        all.push(bundled.clone());
    }
    if let Some(v) = std::env::var_os("FIEX_PLUGIN_DIR") {
        all.push(PathBuf::from(v));
    }
    if let Some(d) = dirs::data_local_dir() {
        all.push(d.join("fiex").join("plugins"));
    }
    let dev_bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
    if dev_bundled.is_dir() {
        all.push(dev_bundled);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            all.push(dir.join("plugins"));
            all.push(dir.to_path_buf());
        }
    }
    // Development-only source manifests, never a special official-only runtime.
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../plugins/official");
    if dev.is_dir() {
        all.push(dev);
    }
    all
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
            if OFFICIAL_IDS.contains(&manifest.id.as_str()) && !trusted_builtin(&manifest, &folder)
            {
                // Reserved IDs cannot be installed by untrusted packages.
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
    let os = match std::env::consts::OS {
        "macos" => "macos",
        "windows" => "windows",
        "linux" => "linux",
        s => s,
    };
    format!("{os}-{}", std::env::consts::ARCH)
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
    // Never look for a matching filename in another package or PATH:
    // doing so would run an attacker binary with an official manifest.
    Err("plugin native backend is not installed inside its package".into())
}
pub fn catalog() -> Vec<PluginInfo> {
    discover()
        .into_iter()
        .map(|(manifest, folder)| {
            let backend = executable(&manifest, &folder).is_ok();
            let official = trusted_builtin(&manifest, &folder);
            let declared = declared(&manifest).unwrap_or_default();
            let state = crate::plugin_permissions::status(&manifest.id, &declared, official)
                .unwrap_or_default(); // unreadable policy fails closed
            PluginInfo {
                id: manifest.id,
                name: manifest.name,
                version: manifest.version,
                capabilities: manifest.capabilities,
                contributes: manifest.contributes,
                backend_available: backend,
                enabled: state.enabled && state.approved,
                official,
                approved: state.approved,
                declared_permissions: declared.into_iter().collect(),
                granted_permissions: state.granted.into_iter().collect(),
            }
        })
        .collect()
}
pub fn exists(id: &str) -> bool {
    discover().iter().any(|(m, _)| m.id == id)
}
struct Running {
    plugin_id: String,
    declared: std::collections::BTreeSet<String>,
    official: bool,
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
            crate::plugin_permissions::allowed(&self.plugin_id, &self.declared, self.official, name)
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
            let official = trusted_builtin(&manifest, &dir);
            let state = crate::plugin_permissions::status(plugin_id, &declared, official)?;
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
                official,
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
