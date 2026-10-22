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
pub fn set_bundle_root(resource_root: PathBuf) {
    let _ = BUNDLED_ROOT.set(resource_root.join("plugins"));
}
fn roots() -> Vec<PathBuf> {
    let mut all = Vec::new();
    if let Some(v) = std::env::var_os("FIEX_PLUGIN_DIR") {
        all.push(PathBuf::from(v));
    }
    if let Some(d) = dirs::data_local_dir() {
        all.push(d.join("fiex").join("plugins"));
    }
    if let Some(bundled) = BUNDLED_ROOT.get() {
        all.push(bundled.clone());
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
            if !valid_id(&manifest.id) || manifest.name.len() > 128 {
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
    if inside.is_file() {
        return Ok(inside);
    }
    // Side-by-side binary during development or bundled deployment.
    let basename = path.file_name().ok_or("invalid executable basename")?;
    for p in roots() {
        let sibling = p.join(basename);
        if sibling.is_file() {
            return Ok(sibling);
        }
    }
    Err("plugin native backend is not installed".into())
}
pub fn catalog() -> Vec<PluginInfo> {
    discover()
        .into_iter()
        .map(|(manifest, folder)| {
            let backend = executable(&manifest, &folder).is_ok();
            PluginInfo {
                id: manifest.id,
                name: manifest.name,
                version: manifest.version,
                capabilities: manifest.capabilities,
                contributes: manifest.contributes,
                backend_available: backend,
                enabled: true,
            }
        })
        .collect()
}
pub fn exists(id: &str) -> bool {
    discover().iter().any(|(m, _)| m.id == id)
}
struct Running {
    plugin_id: String,
    permissions: Value,
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}
impl Running {
    /// All host.* requests are authenticated by the Running process identity,
    /// never by a pluginId supplied in the message.
    fn host_method(&self, method: &str, p: &Value) -> Result<Value, String> {
        let permitted =
            |name: &str| self.permissions.get(name).and_then(Value::as_bool) == Some(true);
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
                permissions: manifest.permissions,
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
