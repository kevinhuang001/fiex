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
fn roots() -> Vec<PathBuf> {
    let mut all = Vec::new();
    if let Some(v) = std::env::var_os("FIEX_PLUGIN_DIR") {
        all.push(PathBuf::from(v));
    }
    if let Some(d) = dirs::data_local_dir() {
        all.push(d.join("fiex").join("plugins"));
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
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}
impl Running {
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
        let response = plugin_sdk::read_frame(&mut self.stdout)
            .map_err(|e| e.to_string())?
            .ok_or("plugin exited during request")?;
        if response.get("id").and_then(Value::as_u64) != Some(id) {
            return Err("plugin response ID mismatch".into());
        }
        if let Some(error) = response.get("error") {
            return Err(format!("plugin error: {error}"));
        }
        response
            .get("result")
            .cloned()
            .ok_or_else(|| "plugin response missing result".into())
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
