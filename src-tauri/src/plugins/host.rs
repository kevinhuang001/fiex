//! Manifest-driven installed-plugin discovery. Every plugin must be approved
//! by the user; no ID, publisher name or package location grants privileges.
use crate::plugin_sdk;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs,
    io::{BufReader, Read, Write},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex, OnceLock},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRuntime {
    #[serde(default)]
    pub protocol_version: String,
    #[serde(default)]
    pub entrypoints: HashMap<String, String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilesystemPermission {
    #[serde(default)]
    pub read: bool,
    #[serde(default)]
    pub write: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandsPermission {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub allow: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkPermission {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub domains: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginPermissions {
    #[serde(default)]
    pub filesystem: FilesystemPermission,
    #[serde(default)]
    pub network: NetworkPermission,
    #[serde(default)]
    pub commands: CommandsPermission,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub runtime: PluginRuntime,
    #[serde(default)]
    pub permissions: PluginPermissions,
    #[serde(default)]
    pub contributions: Value,
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
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub icon: Option<String>,
    pub capabilities: Vec<String>,
    pub contributions: Value,
    pub backend_available: bool,
    pub enabled: bool,
    pub author: Option<PluginAuthor>,
    pub approved: bool,
    pub declared_permissions: Vec<String>,
    pub granted_permissions: Vec<String>,
    pub requested_network_domains: Vec<String>,
    pub allowed_network_domains: Vec<String>,
    pub requested_commands: Vec<String>,
    pub allowed_commands: Vec<String>,
}
pub fn declared_permissions(
    manifest: &PluginManifest,
) -> Result<std::collections::BTreeSet<String>, String> {
    let mut declared = std::collections::BTreeSet::new();
    if manifest.permissions.filesystem.read {
        declared.insert("filesystem.read".to_string());
    }
    if manifest.permissions.filesystem.write {
        declared.insert("filesystem.write".to_string());
    }
    if manifest.permissions.network.enabled {
        declared.insert("network".to_string());
    }
    if manifest.permissions.commands.enabled {
        declared.insert("commands".to_string());
    }
    crate::plugin_permissions::validate_commands(&declared, &manifest.permissions.commands.allow)?;
    crate::plugin_permissions::validate_network_domains(
        &declared,
        &manifest.permissions.network.domains,
    )?;
    Ok(declared)
}

fn declared(manifest: &PluginManifest) -> Result<std::collections::BTreeSet<String>, String> {
    declared_permissions(manifest)
}

fn capabilities(contributions: &Value) -> Vec<String> {
    let mut values = Vec::new();
    for (key, capability) in [
        ("connections", "connection"),
        ("toolbar", "toolbar"),
        ("contextMenus", "contextMenu"),
        ("search", "search"),
        ("settings", "settings"),
    ] {
        let present = contributions
            .get(key)
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty());
        if present {
            values.push(capability.to_string());
        }
    }
    let has_scheme = ["connections", "toolbar"].iter().any(|key| {
        contributions
            .get(key)
            .and_then(Value::as_array)
            .is_some_and(|items| {
                items.iter().any(|item| {
                    item.get("scheme").and_then(Value::as_str).is_some()
                        || item
                            .get("schemes")
                            .and_then(Value::as_array)
                            .is_some_and(|values| !values.is_empty())
                })
            })
    });
    if has_scheme {
        values.push("vfs".to_string());
    }
    let has_panel = ["toolbar", "contextMenus"].iter().any(|key| {
        contributions
            .get(key)
            .and_then(Value::as_array)
            .is_some_and(|items| items.iter().any(|item| item.get("panel").is_some()))
    });
    if has_panel {
        values.push("panel".to_string());
    }
    values
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
pub fn set_network_domain(id: &str, domain: &str, grant: bool) -> Result<(), String> {
    let (manifest, _) = discover()
        .into_iter()
        .find(|(m, _)| m.id == id)
        .ok_or("plugin not installed")?;
    crate::plugin_permissions::set_domain(id, &declared(&manifest)?, domain, grant)?;
    global().stop(id);
    Ok(())
}
pub fn set_command(id: &str, command: &str, grant: bool) -> Result<(), String> {
    let (manifest, _) = discover()
        .into_iter()
        .find(|(m, _)| m.id == id)
        .ok_or("plugin not installed")?;
    crate::plugin_permissions::set_command(id, &declared(&manifest)?, command, grant)?;
    global().stop(id);
    Ok(())
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
    crate::plugin_permissions::approve_install_with_domains(
        id,
        &declared(&manifest)?,
        &manifest.permissions.network.domains,
        &manifest.permissions.commands.allow,
    )?;
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
            if manifest.schema_version != 1
                || manifest.runtime.protocol_version != plugin_sdk::PROTOCOL_VERSION
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
    discover()
        .into_iter()
        .map(|(manifest, folder)| {
            let backend = executable(&manifest, &folder).is_ok();
            let declared = declared(&manifest).unwrap_or_default();
            let state =
                crate::plugin_permissions::status(&manifest.id, &declared).unwrap_or_default(); // unreadable policy fails closed
            let capabilities = capabilities(&manifest.contributions);
            PluginInfo {
                schema_version: manifest.schema_version,
                id: manifest.id,
                name: manifest.name,
                version: manifest.version,
                description: manifest.description,
                icon: manifest.icon,
                capabilities,
                contributions: manifest.contributions,
                backend_available: backend,
                enabled: state.enabled && state.approved,
                author: manifest.author,
                approved: state.approved,
                declared_permissions: declared.into_iter().collect(),
                granted_permissions: state.granted.into_iter().collect(),
                requested_network_domains: manifest.permissions.network.domains,
                allowed_network_domains: state.network_domains.into_iter().collect(),
                requested_commands: manifest.permissions.commands.allow,
                allowed_commands: state.commands.into_iter().collect(),
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
    let panel = ["toolbar", "contextMenus"]
        .iter()
        .filter_map(|key| manifest.contributions.get(key).and_then(Value::as_array))
        .flatten()
        .find(|item| item.get("id").and_then(Value::as_str) == Some(panel_id))
        .and_then(|item| item.get("panel"))
        .ok_or("plugin declares no such panel")?;
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
const EXEC_OUTPUT_LIMIT: usize = 2 * 1024 * 1024;
const EXEC_TIMEOUT: Duration = Duration::from_secs(120);

fn read_capped(mut reader: impl Read) -> (Vec<u8>, bool) {
    let mut output = Vec::new();
    let mut truncated = false;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                if output.len() + read > EXEC_OUTPUT_LIMIT {
                    let keep = EXEC_OUTPUT_LIMIT.saturating_sub(output.len());
                    output.extend_from_slice(&buffer[..keep]);
                    truncated = true;
                    break;
                }
                output.extend_from_slice(&buffer[..read]);
            }
            Err(_) => break,
        }
    }
    (output, truncated)
}

fn host_exec(
    plugin_id: &str,
    declared: &std::collections::BTreeSet<String>,
    params: &Value,
) -> Result<Value, String> {
    let command = params
        .get("command")
        .and_then(Value::as_str)
        .ok_or("missing command")?;
    if !crate::plugin_permissions::valid_command(command)
        || !crate::plugin_permissions::allowed_command(plugin_id, declared, command)?
    {
        return Err("command was not approved by the user".into());
    }
    let args = params
        .get("args")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| "command arguments must be strings".to_string())
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    if args.len() > 128
        || args
            .iter()
            .any(|arg| arg.len() > 8192 || arg.contains('\0'))
    {
        return Err("command arguments are too large".into());
    }
    let cwd = params
        .get("cwd")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .map(|path| dunce::canonicalize(path).map_err(|error| error.to_string()))
        .transpose()?;
    if cwd.as_ref().is_some_and(|path| !path.is_dir()) {
        return Err("command working directory is not a folder".into());
    }
    let executable =
        which::which(command).map_err(|error| format!("command unavailable: {error}"))?;
    let mut process = Command::new(executable);
    process
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear()
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "Never");
    for key in [
        "PATH",
        "HOME",
        "USERPROFILE",
        "LANG",
        "LC_ALL",
        "TMPDIR",
        "TEMP",
        "TMP",
        "SystemRoot",
        "PATHEXT",
        "SSH_AUTH_SOCK",
    ] {
        if let Some(value) = std::env::var_os(key) {
            process.env(key, value);
        }
    }
    if let Some(cwd) = &cwd {
        process.current_dir(cwd);
    }
    let mut child = process
        .spawn()
        .map_err(|error| format!("command failed to start: {error}"))?;
    if let Some(stdin) = params.get("stdin").and_then(Value::as_str) {
        if let Some(mut pipe) = child.stdin.take() {
            pipe.write_all(stdin.as_bytes())
                .map_err(|error| error.to_string())?;
        }
    } else {
        drop(child.stdin.take());
    }
    let out_thread = child
        .stdout
        .take()
        .map(|stream| thread::spawn(move || read_capped(stream)));
    let err_thread = child
        .stderr
        .take()
        .map(|stream| thread::spawn(move || read_capped(stream)));
    let deadline = Instant::now() + EXEC_TIMEOUT;
    let status = loop {
        match child.try_wait().map_err(|error| error.to_string())? {
            Some(status) => break status,
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("command timed out".into());
            }
            None => thread::sleep(Duration::from_millis(20)),
        }
    };
    let (stdout, out_truncated) = out_thread
        .map(|handle| handle.join().unwrap_or_default())
        .unwrap_or_default();
    let (stderr, err_truncated) = err_thread
        .map(|handle| handle.join().unwrap_or_default())
        .unwrap_or_default();
    use base64::Engine;
    let engine = base64::engine::general_purpose::STANDARD;
    Ok(json!({
        "code": status.code(),
        "stdout": String::from_utf8_lossy(&stdout),
        "stdoutBase64": engine.encode(&stdout),
        "stderr": String::from_utf8_lossy(&stderr),
        "stderrBase64": engine.encode(&stderr),
        "truncated": out_truncated || err_truncated
    }))
}

struct Running {
    plugin_id: String,
    declared: std::collections::BTreeSet<String>,
    child: crate::plugin_sandbox::SandboxChild,
    stdin: Box<dyn Write + Send>,
    stdout: BufReader<Box<dyn Read + Send>>,
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
            "host.storage.get" => {
                Ok(
                    crate::plugin_storage::get(&self.plugin_id, scope()?, key()?)?
                        .unwrap_or(Value::Null),
                )
            }
            "host.storage.set" => {
                let value = p.get("value").cloned().ok_or("missing value")?;
                crate::plugin_storage::set(&self.plugin_id, scope()?, key()?, value)?;
                Ok(Value::Null)
            }
            "host.storage.delete" => {
                crate::plugin_storage::delete(&self.plugin_id, scope()?, key()?)?;
                Ok(Value::Null)
            }
            "host.storage.keys" => Ok(json!(crate::plugin_storage::list_keys(
                &self.plugin_id,
                scope()?
            )?)),
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
            "host.exec" if permitted("commands") => host_exec(&self.plugin_id, &self.declared, p),
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
        // Dropping SandboxChild terminates the plugin and its process tree.
        let _ = &self.child;
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
            let policy = crate::plugin_sandbox::Policy::new(&path, &dir, &state, &declared);
            let mut sandbox = crate::plugin_sandbox::spawn(&policy)?;
            let stdin = std::mem::replace(
                &mut sandbox.stdin,
                Box::new(std::io::sink()) as Box<dyn Write + Send>,
            );
            let stdout = std::mem::replace(
                &mut sandbox.stdout,
                BufReader::new(Box::new(std::io::empty()) as Box<dyn Read + Send>),
            );
            let mut running = Running {
                plugin_id: plugin_id.to_string(),
                declared,
                child: sandbox,
                stdin,
                stdout,
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
