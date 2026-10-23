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
pub struct FolderReadPermission {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub decorations: Option<Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderPermission {
    #[serde(default)]
    pub read: FolderReadPermission,
    #[serde(default)]
    pub create: bool,
    #[serde(default)]
    pub delete: bool,
    #[serde(default)]
    pub rename: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePermission {
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
    pub folders: FolderPermission,
    #[serde(default)]
    pub files: FilePermission,
    #[serde(default)]
    pub network: NetworkPermission,
    #[serde(default)]
    pub commands: CommandsPermission,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginPublisher {
    pub id: String,
    pub name: String,
    pub key_id: String,
    pub public_key: String,
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
    #[serde(default)]
    pub publisher: Option<PluginPublisher>,
    #[serde(default)]
    pub files: std::collections::BTreeMap<String, String>,
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
    /// Requested commands that cannot be resolved on PATH right now. The
    /// settings UI marks these red: the plugin is approved to run them but the
    /// machine cannot.
    pub missing_commands: Vec<String>,
}
pub fn declared_permissions(
    manifest: &PluginManifest,
) -> Result<std::collections::BTreeSet<String>, String> {
    let mut declared = std::collections::BTreeSet::new();
    let folders = &manifest.permissions.folders;
    let files = &manifest.permissions.files;
    if folders.read.enabled {
        declared.insert("folders.read".to_string());
    }
    for (name, requested) in [
        ("folders.create", folders.create),
        ("folders.delete", folders.delete),
        ("folders.rename", folders.rename),
    ] {
        if requested {
            declared.insert(name.into());
        }
    }
    if files.read {
        declared.insert("files.read".to_string());
    }
    if files.write {
        declared.insert("files.write".to_string());
    }
    if manifest.permissions.network.enabled || !manifest.permissions.network.domains.is_empty() {
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
        ("themes", "theme"),
        ("preview", "preview"),
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
    let required = activation_requirements(&manifest)?;
    if enabled
        && !crate::plugin_permissions::activation_complete(
            &crate::plugin_permissions::status(id, &required.permissions)?,
            &required,
        )
    {
        return Err("review and approve every requested permission before activation".into());
    }
    crate::plugin_permissions::set_enabled(id, &required.permissions, enabled)?;
    global().stop(id);
    Ok(())
}
pub fn activation_requirements(
    manifest: &PluginManifest,
) -> Result<crate::plugin_permissions::ActivationConsent, String> {
    let permissions = declared(manifest)?;
    let domains = crate::plugin_permissions::validate_network_domains(
        &permissions,
        &manifest.permissions.network.domains,
    )?;
    let commands = crate::plugin_permissions::validate_commands(
        &permissions,
        &manifest.permissions.commands.allow,
    )?;
    Ok(crate::plugin_permissions::ActivationConsent {
        permissions,
        domains,
        commands,
    })
}

pub fn activate(
    id: &str,
    consent: &crate::plugin_permissions::ActivationConsent,
) -> Result<(), String> {
    let (manifest, _) = discover()
        .into_iter()
        .find(|(m, _)| m.id == id)
        .ok_or("plugin not installed")?;
    crate::plugin_permissions::activate(id, &activation_requirements(&manifest)?, consent)?;
    global().stop(id);
    Ok(())
}

pub fn declared_permissions_for(id: &str) -> Result<std::collections::BTreeSet<String>, String> {
    let (manifest, _) = discover()
        .into_iter()
        .find(|(m, _)| m.id == id)
        .ok_or("plugin not installed")?;
    declared(&manifest)
}

pub fn installed_manifests() -> Vec<(PluginManifest, PathBuf)> {
    discover()
}

pub fn network_domains(id: &str) -> Result<Vec<String>, String> {
    let (manifest, _) = discover()
        .into_iter()
        .find(|(m, _)| m.id == id)
        .ok_or("plugin not installed")?;
    let declared = declared(&manifest)?;
    Ok(crate::plugin_permissions::status(id, &declared)?
        .network_domains
        .into_iter()
        .collect())
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
                || (!crate::plugin_themes::is_theme(&manifest)
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
/// Commands Core would fail to spawn because they are not on PATH today.
fn missing_commands(commands: &std::collections::BTreeSet<String>) -> Vec<String> {
    commands
        .iter()
        .filter(|command| which::which(command).is_err())
        .cloned()
        .collect()
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
            let backend =
                crate::plugin_themes::is_theme(&manifest) || executable(&manifest, &folder).is_ok();
            let declared = declared(&manifest).unwrap_or_default();
            let state =
                crate::plugin_permissions::status(&manifest.id, &declared).unwrap_or_default(); // unreadable policy fails closed
            let active = activation_requirements(&manifest).is_ok_and(|required| {
                state.enabled && crate::plugin_permissions::activation_complete(&state, &required)
            });
            let requested = activation_requirements(&manifest).unwrap_or_default();
            let missing_commands = missing_commands(&requested.commands);
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
                enabled: active,
                author: manifest.author,
                approved: state.approved,
                declared_permissions: declared.into_iter().collect(),
                granted_permissions: state.granted.into_iter().collect(),
                requested_network_domains: requested.domains.into_iter().collect(),
                allowed_network_domains: state.network_domains.into_iter().collect(),
                requested_commands: requested.commands.into_iter().collect(),
                allowed_commands: state.commands.into_iter().collect(),
                missing_commands,
            }
        })
        .collect()
}
/// The plugin document must define its own policy. It is served from the
/// asset origin rather than injected into the app shell, so the app's stricter
/// `script-src 'self'` does not block the plugin's inline bundle.
const PANEL_CSP: &str = "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:; connect-src 'none'; form-action 'none'; base-uri 'none'\">";
fn panel_document(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let insert = lower
        .find("<head")
        .and_then(|start| lower[start..].find('>').map(|offset| start + offset + 1));
    match insert {
        Some(index) => format!("{}{PANEL_CSP}{}", &html[..index], &html[index..]),
        None => format!("{PANEL_CSP}{html}"),
    }
}
/// A panel is inlined on a toolbar or context-menu entry, possibly inside a
/// context submenu. Search mirrors the frontend's depth limit so the two never
/// disagree about which ids are openable.
fn find_panel<'a>(manifest: &'a PluginManifest, panel_id: &str) -> Option<&'a Value> {
    fn walk<'a>(items: &'a [Value], panel_id: &str, depth: usize) -> Option<&'a Value> {
        if depth > 4 {
            return None;
        }
        for item in items {
            if item.get("id").and_then(Value::as_str) == Some(panel_id) {
                if let Some(panel) = item.get("panel") {
                    return Some(panel);
                }
            }
            let nested = item.get("children").and_then(Value::as_array);
            if let Some(found) = nested.and_then(|children| walk(children, panel_id, depth + 1)) {
                return Some(found);
            }
        }
        None
    }
    ["toolbar", "contextMenus", "preview"]
        .iter()
        .filter_map(|key| manifest.contributions.get(key).and_then(Value::as_array))
        .find_map(|items| walk(items, panel_id, 1))
}
/// Host-approved precompiled, self-contained iframe HTML. Plugins cannot
/// supply absolute paths or read unrelated files through this API.
fn panel_source(plugin_id: &str, panel_id: &str) -> Result<String, String> {
    let (manifest, folder) = discover()
        .into_iter()
        .find(|(m, _)| m.id == plugin_id)
        .ok_or("plugin is not installed")?;
    let rights = crate::plugin_permissions::status(plugin_id, &declared(&manifest)?)?;
    if !rights.enabled
        || !crate::plugin_permissions::activation_complete(
            &rights,
            &activation_requirements(&manifest)?,
        )
    {
        return Err("plugin disabled or not approved".into());
    }
    let panel = find_panel(&manifest, panel_id).ok_or("plugin declares no such panel")?;
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
pub fn panel_html(plugin_id: &str, panel_id: &str) -> Result<String, String> {
    panel_source(plugin_id, panel_id)
}
/// Desktop panels are served from the asset protocol with their own CSP. A
/// `srcDoc` document inherits the app shell's CSP, which blocks inline plugin
/// bundles in packaged builds. The cache copy is content-addressed.
pub fn panel_asset(plugin_id: &str, panel_id: &str) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let html = panel_document(&panel_source(plugin_id, panel_id)?);
    let digest = format!("{:x}", Sha256::digest(html.as_bytes()));
    let directory = dirs::cache_dir()
        .ok_or("cache directory unavailable")?
        .join("fiex")
        .join("plugin-panels")
        .join(plugin_id);
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let safe_panel = panel_id
        .chars()
        .map(|value| {
            if value.is_ascii_alphanumeric() || value == '-' || value == '_' {
                value
            } else {
                '_'
            }
        })
        .collect::<String>();
    let path = directory.join(format!("{safe_panel}-{}.html", &digest[..16]));
    fs::write(&path, html).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}
pub fn exists(id: &str) -> bool {
    discover().iter().any(|(m, _)| m.id == id)
}
const EXEC_OUTPUT_LIMIT: usize = 1024 * 1024;
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
        "GNUPGHOME",
        "GPG_TTY",
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "DBUS_SESSION_BUS_ADDRESS",
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

pub fn run_command(
    plugin_id: &str,
    declared: &std::collections::BTreeSet<String>,
    params: &Value,
    task_root: Option<&Path>,
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
    let cwd = task_root
        .map(Path::to_path_buf)
        .unwrap_or(std::env::current_dir().map_err(|e| e.to_string())?);
    if !cwd.is_dir() {
        return Err("active directory is unavailable".into());
    }
    let executable =
        which::which(command).map_err(|error| format!("command unavailable: {error}"))?;
    let mut process = Command::new(executable);
    process
        .args(&args)
        .current_dir(&cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear()
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "Never")
        .env("LC_ALL", "C");
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
        "GNUPGHOME",
        "GPG_TTY",
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "DBUS_SESSION_BUS_ADDRESS",
    ] {
        if let Some(value) = std::env::var_os(key) {
            process.env(key, value);
        }
    }
    let mut child = process
        .spawn()
        .map_err(|error| format!("command failed to start: {error}"))?;
    let input = params.get("stdin").and_then(Value::as_str).unwrap_or("");
    if input.len() > 1024 * 1024 {
        let _ = child.kill();
        let _ = child.wait();
        return Err("command input is too large".into());
    }
    let (input_sender, input_receiver) = std::sync::mpsc::sync_channel(1);
    let input = input.to_owned();
    let input_pipe = child.stdin.take();
    thread::spawn(move || {
        let result = input_pipe.map_or(Ok(()), |mut pipe| pipe.write_all(input.as_bytes()));
        let _ = input_sender.send(result);
    });
    let capture = |stream: Option<std::process::ChildStdout>| {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        thread::spawn(move || {
            let value = stream.map(read_capped).unwrap_or_default();
            let _ = sender.send(value);
        });
        receiver
    };
    let out_receiver = capture(child.stdout.take());
    let (err_sender, err_receiver) = std::sync::mpsc::sync_channel(1);
    let err_stream = child.stderr.take();
    thread::spawn(move || {
        let value = err_stream.map(read_capped).unwrap_or_default();
        let _ = err_sender.send(value);
    });
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
    let remaining = || deadline.saturating_duration_since(Instant::now());
    input_receiver
        .recv_timeout(remaining())
        .map_err(|_| "command input timed out")?
        .map_err(|error| error.to_string())?;
    let (stdout, out_truncated) = out_receiver
        .recv_timeout(remaining())
        .map_err(|_| "command output timed out")?;
    let (stderr, err_truncated) = err_receiver
        .recv_timeout(remaining())
        .map_err(|_| "command error output timed out")?;
    use base64::Engine;
    Ok(json!({
        "code": status.code(),
        "stdoutBase64": base64::engine::general_purpose::STANDARD.encode(&stdout),
        "stderrBase64": base64::engine::general_purpose::STANDARD.encode(&stderr),
        "truncated": out_truncated || err_truncated
    }))
}

struct Running {
    plugin_id: String,
    resource_owner: String,
    declared: std::collections::BTreeSet<String>,
    child: crate::plugin_sandbox::SandboxChild,
    stdin: Box<dyn Write + Send>,
    stdout: std::sync::mpsc::Receiver<Result<Option<Value>, String>>,
    next_id: u64,
    task_root: Option<crate::plugin_fs_access::Scope>,
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
        let root = || {
            self.task_root
                .as_ref()
                .ok_or_else(|| "no task directory".to_string())
        };
        let scope = || arg("scope");
        let key = || arg("key");
        match method {
            "host.net.http.open" if permitted("network") => crate::plugin_net::http_open(
                &self.resource_owner,
                &self.plugin_id,
                &self.declared,
                p,
            ),
            "host.net.http.read" if permitted("network") => {
                crate::plugin_net::http_read(&self.resource_owner, p)
            }
            "host.net.http.close" => {
                crate::plugin_net::http_close(&self.resource_owner, p)?;
                Ok(Value::Null)
            }
            "host.net.http" if permitted("network") => {
                crate::plugin_net::http(&self.resource_owner, &self.plugin_id, &self.declared, p)
            }
            "host.net.tcp.open" if permitted("network") => crate::plugin_net::tcp_open(
                &self.resource_owner,
                &self.plugin_id,
                &self.declared,
                p,
            ),
            "host.net.tcp.readBytes" if permitted("network") => {
                crate::plugin_net::tcp_read_bytes(&self.resource_owner, p)
            }
            "host.net.tcp.writeBytes" if permitted("network") => {
                crate::plugin_net::tcp_write_bytes(&self.resource_owner, p)
            }
            "host.net.tcp.read" if permitted("network") => {
                crate::plugin_net::tcp_read(&self.resource_owner, p)
            }
            "host.net.tcp.write" if permitted("network") => {
                crate::plugin_net::tcp_write(&self.resource_owner, p)
            }
            "host.net.tcp.close" if permitted("network") => {
                crate::plugin_net::tcp_close(&self.resource_owner, p)?;
                Ok(Value::Null)
            }
            "host.preview.create" => {
                use base64::Engine;
                let name = arg("name")?;
                if name.is_empty()
                    || !Path::new(name)
                        .components()
                        .all(|c| matches!(c, Component::Normal(_)))
                    || Path::new(name).components().count() != 1
                {
                    return Err("invalid preview name".into());
                }
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(arg("base64")?)
                    .map_err(|e| e.to_string())?;
                if bytes.len() > 1024 * 1024 {
                    return Err("preview too large".into());
                }
                let directory = dirs::cache_dir()
                    .ok_or("cache directory unavailable")?
                    .join("fiex")
                    .join("plugin-previews")
                    .join(&self.plugin_id);
                fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
                let path = directory.join(name);
                fs::write(&path, bytes).map_err(|e| e.to_string())?;
                Ok(json!(path.to_string_lossy()))
            }
            "host.buffer.create" => {
                let size = p
                    .get("size")
                    .and_then(Value::as_u64)
                    .ok_or("missing size")? as usize;
                let writable = p.get("writable").and_then(Value::as_bool).unwrap_or(false);
                serde_json::to_value(crate::plugin_ipc::create(
                    &self.resource_owner,
                    size,
                    writable,
                )?)
                .map_err(|error| error.to_string())
            }
            "host.buffer.map" => {
                let id = p.get("id").and_then(Value::as_str).ok_or("missing id")?;
                serde_json::to_value(crate::plugin_ipc::map(&self.resource_owner, id)?)
                    .map_err(|error| error.to_string())
            }
            "host.fs.readChunk" if permitted("files.read") => crate::plugin_fs_access::read_chunk(
                root()?,
                arg("path")?,
                p.get("offset").and_then(Value::as_u64).unwrap_or(0),
                p.get("length").and_then(Value::as_u64).unwrap_or(262144) as usize,
            ),
            "host.fs.mkdir" if permitted("folders.create") => {
                crate::plugin_fs_access::mkdir(root()?, arg("path")?)
            }
            "host.fs.beginWrite" if permitted("files.write") => {
                let create = p.get("create").and_then(Value::as_bool).unwrap_or(false);
                if create && !permitted("folders.create") {
                    return Err(
                        "creating a file requires directory entry creation permission".into(),
                    );
                }
                Ok(json!(crate::plugin_fs_access::begin_write(
                    &self.resource_owner,
                    root()?,
                    arg("path")?,
                    create
                )?))
            }
            "host.fs.seekWrite" if permitted("files.write") => {
                Ok(json!(crate::plugin_fs_access::seek_write(
                    &self.resource_owner,
                    arg("handle")?,
                    arg("kind")?,
                    p.get("offset")
                        .and_then(Value::as_i64)
                        .ok_or("missing seek offset")?
                )?))
            }
            "host.fs.writeChunk" if permitted("files.write") => {
                crate::plugin_fs_access::write_chunk(
                    &self.resource_owner,
                    arg("handle")?,
                    arg("base64")?,
                )?;
                Ok(Value::Null)
            }
            "host.fs.commit" if permitted("files.write") => {
                crate::plugin_fs_access::commit(&self.resource_owner, arg("handle")?)
            }
            "host.fs.abort" => {
                crate::plugin_fs_access::abort(&self.resource_owner, arg("handle")?)?;
                Ok(Value::Null)
            }
            "host.fs.list" if permitted("folders.read") => {
                let path = p.get("path").and_then(Value::as_str).unwrap_or(".");
                crate::plugin_fs_access::list(root()?, path)
            }
            "host.fs.stat" if permitted("folders.read") || permitted("files.read") => {
                let path = p
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or("missing path")?;
                crate::plugin_fs_access::stat(root()?, path)
            }
            "host.fs.read" if permitted("files.read") => {
                let path = p
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or("missing path")?;
                let offset = p.get("offset").and_then(Value::as_u64).unwrap_or(0);
                let length = p
                    .get("length")
                    .and_then(Value::as_u64)
                    .unwrap_or(1024 * 1024) as usize;
                crate::plugin_fs_access::read(&self.resource_owner, root()?, path, offset, length)
            }
            "host.fs.create" if permitted("folders.create") => {
                let path = p
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or("missing path")?;
                crate::plugin_fs_access::create(root()?, path)
            }
            "host.fs.write" if permitted("files.write") => {
                let path = p
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or("missing path")?;
                let buffer = p
                    .get("buffer")
                    .and_then(Value::as_str)
                    .ok_or("missing buffer")?;
                let mode = p.get("mode").and_then(Value::as_str).unwrap_or("truncate");
                if mode == "create" && !permitted("folders.create") {
                    return Err("folder write permission is required to create a file".into());
                }
                crate::plugin_fs_access::write(&self.resource_owner, root()?, path, buffer, mode)
            }
            "host.fs.remove" if permitted("folders.delete") => {
                let path = p
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or("missing path")?;
                crate::plugin_fs_access::remove(root()?, path)?;
                Ok(Value::Null)
            }
            "host.fs.rename" if permitted("folders.rename") => {
                let from = p
                    .get("from")
                    .and_then(Value::as_str)
                    .ok_or("missing from")?;
                let to = p.get("to").and_then(Value::as_str).ok_or("missing to")?;
                crate::plugin_fs_access::rename(root()?, from, to)?;
                Ok(Value::Null)
            }
            "host.buffer.release" => {
                let id = p.get("id").and_then(Value::as_str).ok_or("missing id")?;
                crate::plugin_ipc::release(&self.resource_owner, id)?;
                Ok(Value::Null)
            }
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
            "host.secret.get" => Ok(json!(crate::plugin_storage::secret_get(
                &self.plugin_id,
                scope()?,
                key()?
            )?)),
            "host.secret.set" => {
                crate::plugin_storage::secret_set(
                    &self.plugin_id,
                    scope()?,
                    key()?,
                    arg("value")?,
                )?;
                Ok(Value::Null)
            }
            "host.secret.delete" => {
                crate::plugin_storage::secret_delete(&self.plugin_id, scope()?, key()?)?;
                Ok(Value::Null)
            }
            "host.secret.available" => Ok(json!(crate::plugin_storage::secret_available())),
            // Diagnostics only: no permission gate, capped and best effort.
            "host.error.report" => {
                let code = p.get("code").and_then(Value::as_str).unwrap_or("");
                let message = p
                    .get("message")
                    .and_then(Value::as_str)
                    .ok_or("missing message")?;
                let detail = p.get("detail").and_then(Value::as_str);
                crate::plugin_errors::report_limited(&self.plugin_id, code, message, detail);
                Ok(Value::Null)
            }
            "host.exec" if permitted("commands") => host_exec(&self.plugin_id, &self.declared, p),
            "host.process.run" if permitted("commands") => run_command(
                &self.plugin_id,
                &self.declared,
                p,
                self.task_root.as_ref().map(|scope| scope.path()),
            ),
            _ => Err("host method not available or permission denied".into()),
        }
    }
    fn rpc(
        &mut self,
        method: &str,
        params: Value,
        root: Option<crate::plugin_fs_access::Scope>,
    ) -> Result<Value, String> {
        self.task_root = root;
        self.next_id += 1;
        let id = self.next_id;
        plugin_sdk::write_frame(
            &mut self.stdin,
            &json!({
                "jsonrpc":"2.0","id":id,"method":method,"params":params
            }),
        )
        .map_err(|e| e.to_string())?;
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or("plugin request deadline exceeded")?;
            let response = self
                .stdout
                .recv_timeout(remaining)
                .map_err(|_| "plugin stopped responding or disconnected")??
                .ok_or_else(|| {
                    format!(
                        "plugin exited during request: {}",
                        self.child.exit_diagnostic()
                    )
                })?;
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
        crate::plugin_fs_access::cleanup(&self.resource_owner);
        crate::plugin_net::cleanup(&self.resource_owner);
        crate::plugin_ipc::cleanup(&self.resource_owner);
        let _ = &self.child;
    }
}
static ACTIVE_DIRECTORY: OnceLock<Mutex<Option<std::path::PathBuf>>> = OnceLock::new();

pub fn active_path() -> Option<std::path::PathBuf> {
    ACTIVE_DIRECTORY
        .get_or_init(|| Mutex::new(None))
        .lock()
        .ok()
        .and_then(|value| value.clone())
}

pub fn set_active_directory(path: &str) -> Result<(), String> {
    let path = dunce::canonicalize(path).map_err(|error| error.to_string())?;
    if !path.is_dir() {
        return Err("active path is not a directory".into());
    }
    *ACTIVE_DIRECTORY
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|_| "active directory lock poisoned")? = Some(path);
    Ok(())
}

struct Instance {
    running: Mutex<Running>,
    kill: crate::plugin_sandbox::KillHandle,
}

#[derive(Default)]
pub struct PluginHost {
    children: Mutex<HashMap<String, Arc<Instance>>>,
}
impl PluginHost {
    pub fn stop(&self, id: &str) {
        if let Ok(mut processes) = self.children.lock() {
            if let Some(instance) = processes.remove(id) {
                instance.kill.terminate();
            }
        }
    }
    pub fn is_installed(&self, id: &str) -> bool {
        discover()
            .into_iter()
            .find(|(m, _)| m.id == id)
            .is_some_and(|(m, d)| executable(&m, &d).is_ok())
    }
    pub fn invoke(&self, plugin_id: &str, method: &str, params: Value) -> Result<Value, String> {
        self.invoke_scoped(plugin_id, method, params, active_path())
    }
    pub fn invoke_at(
        &self,
        plugin_id: &str,
        method: &str,
        params: Value,
        directory: &str,
    ) -> Result<Value, String> {
        let root = if directory.contains("://") {
            None
        } else {
            Some(dunce::canonicalize(directory).map_err(|e| e.to_string())?)
        };
        self.invoke_scoped(plugin_id, method, params, root)
    }
    fn invoke_scoped(
        &self,
        plugin_id: &str,
        method: &str,
        params: Value,
        task_root: Option<PathBuf>,
    ) -> Result<Value, String> {
        let task_root = task_root
            .as_deref()
            .map(crate::plugin_fs_access::Scope::new)
            .transpose()?;
        let (manifest, dir) = discover()
            .into_iter()
            .find(|(m, _)| m.id == plugin_id)
            .ok_or("plugin not installed")?;
        let required = activation_requirements(&manifest)?;
        let state = crate::plugin_permissions::status(plugin_id, &required.permissions)?;
        if !state.enabled || !crate::plugin_permissions::activation_complete(&state, &required) {
            return Err("plugin activation requires complete permission consent".into());
        }
        let mut params = params;
        if let Some(object) = params.as_object_mut() {
            object.insert("networkDomains".into(), json!(state.network_domains));
        }
        let mut guard = self
            .children
            .lock()
            .map_err(|_| "plugin host lock poisoned")?;
        let instance = if let Some(instance) = guard.get(plugin_id) {
            instance.clone()
        } else {
            crate::plugin_signing::verify_installed(&manifest, &dir)?;
            let declared = declared(&manifest)?;
            let state = crate::plugin_permissions::status(plugin_id, &declared)?;
            if !state.enabled
                || !crate::plugin_permissions::activation_complete(
                    &state,
                    &activation_requirements(&manifest)?,
                )
            {
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
            let kill = sandbox.kill_handle();
            let (sender, receiver) = std::sync::mpsc::sync_channel(1);
            thread::spawn(move || {
                let mut stdout = stdout;
                loop {
                    let frame = plugin_sdk::read_frame(&mut stdout).map_err(|e| e.to_string());
                    let finished = !matches!(frame, Ok(Some(_)));
                    if sender.send(frame).is_err() || finished {
                        break;
                    }
                }
            });
            let running = Running {
                plugin_id: plugin_id.to_string(),
                resource_owner: format!(
                    "{plugin_id}:{}:{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_err(|e| e.to_string())?
                        .as_nanos()
                ),
                declared,
                child: sandbox,
                stdin,
                stdout: receiver,
                next_id: 0,
                task_root: None,
            };
            let instance = Arc::new(Instance {
                running: Mutex::new(running),
                kill,
            });
            guard.insert(plugin_id.to_string(), instance.clone());
            instance
        };
        drop(guard);
        let response = (|| {
            let mut running = instance
                .running
                .lock()
                .map_err(|_| "plugin instance lock poisoned")?;
            if running.next_id == 0 {
                let result = running.rpc(
                    "plugin.initialize",
                    json!({"protocolVersion":plugin_sdk::PROTOCOL_VERSION}),
                    task_root.clone(),
                )?;
                if result.get("pluginId").and_then(Value::as_str) != Some(plugin_id) {
                    return Err("plugin identity mismatch".into());
                }
            }
            running.rpc(method, params, task_root)
        })();
        if response.is_err() {
            instance.kill.terminate();
            if let Ok(mut guard) = self.children.lock() {
                if guard
                    .get(plugin_id)
                    .is_some_and(|current| Arc::ptr_eq(current, &instance))
                {
                    guard.remove(plugin_id);
                }
            }
        }
        response
    }
}
pub fn global() -> &'static PluginHost {
    static HOST: OnceLock<PluginHost> = OnceLock::new();
    HOST.get_or_init(PluginHost::default)
}

pub fn themes() -> Vec<crate::plugin_themes::Theme> {
    discover()
        .into_iter()
        .filter(|(manifest, _)| {
            activation_requirements(manifest).is_ok_and(|required| {
                crate::plugin_permissions::status(&manifest.id, &required.permissions).is_ok_and(
                    |state| {
                        state.enabled
                            && crate::plugin_permissions::activation_complete(&state, &required)
                    },
                )
            })
        })
        .flat_map(|(manifest, folder)| {
            crate::plugin_themes::load(&manifest, &folder).unwrap_or_default()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn manifest(contributions: Value) -> PluginManifest {
        serde_json::from_value(json!({
            "schemaVersion": 1,
            "id": "test.panels",
            "name": "Panels",
            "version": "1.0.0",
            "contributions": contributions
        }))
        .unwrap()
    }

    #[test]
    fn resolves_panels_nested_in_context_submenus() {
        let manifest = manifest(json!({
            "contextMenus": [{
                "id": "archive.compress",
                "title": "压缩",
                "when": "context.selection.count > 0",
                "children": [{
                    "id": "archive.advanced",
                    "title": "高级压缩…",
                    "panel": { "entry": "ui/dist/index.html", "title": "高级压缩" }
                }]
            }]
        }));
        let panel = find_panel(&manifest, "archive.advanced").unwrap();
        assert_eq!(panel["title"], "高级压缩");
    }

    #[test]
    fn resolves_toolbar_panels_and_rejects_unknown_ids() {
        let manifest = manifest(json!({
            "toolbar": [{ "id": "git.main", "title": "Git", "panel": { "entry": "ui/git.html" } }]
        }));
        assert_eq!(
            find_panel(&manifest, "git.main").unwrap()["entry"],
            "ui/git.html"
        );
        assert!(find_panel(&manifest, "missing").is_none());
    }

    #[test]
    fn entry_ids_without_a_panel_do_not_resolve() {
        let manifest = manifest(json!({
            "contextMenus": [{ "id": "gpg.menu", "title": "GPG", "command": "gpg.run" }]
        }));
        assert!(find_panel(&manifest, "gpg.menu").is_none());
    }

    #[test]
    fn panel_ids_deeper_than_the_frontend_limit_do_not_resolve() {
        let mut item = json!({
            "id": "deep.panel",
            "title": "Deep",
            "panel": { "entry": "ui/index.html" }
        });
        for _ in 0..5 {
            item = json!({ "id": "level", "title": "Level", "children": [item] });
        }
        let manifest = manifest(json!({ "contextMenus": [item] }));
        assert!(find_panel(&manifest, "deep.panel").is_none());
    }

    #[test]
    fn panel_ids_at_the_frontend_depth_limit_still_resolve() {
        let mut item = json!({
            "id": "fourth.level",
            "title": "Fourth",
            "panel": { "entry": "ui/fourth.html" }
        });
        // Three wrappers put the panel on the fourth level, the deepest the
        // frontend accepts.
        for _ in 0..3 {
            item = json!({ "id": "level", "title": "Level", "children": [item] });
        }
        let manifest = manifest(json!({ "contextMenus": [item] }));
        assert_eq!(
            find_panel(&manifest, "fourth.level").unwrap()["entry"],
            "ui/fourth.html"
        );
    }

    #[test]
    fn submenu_parents_without_panels_do_not_resolve_as_panels() {
        let manifest = manifest(json!({
            "contextMenus": [{
                "id": "gpg.menu",
                "title": "GPG",
                "children": [{ "id": "gpg.encrypt", "title": "加密…", "panel": { "entry": "ui/index.html" } }]
            }]
        }));
        assert!(find_panel(&manifest, "gpg.menu").is_none());
        assert!(find_panel(&manifest, "gpg.encrypt").is_some());
    }

    #[test]
    fn preview_panels_resolve_like_toolbar_panels() {
        let manifest: PluginManifest = serde_json::from_value(json!({
            "schemaVersion": 1,
            "id": "org.test.preview",
            "name": "Preview",
            "version": "0.1.0",
            "description": "fixture",
            "runtime": { "protocolVersion": "2.0", "entrypoints": {} },
            "permissions": {},
            "contributions": {
                "preview": [{
                    "id": "docx",
                    "title": "Word",
                    "extensions": ["docx"],
                    "priority": 100,
                    "panel": { "entry": "ui/index.html" }
                }]
            }
        }))
        .unwrap();
        let panel = find_panel(&manifest, "docx").expect("preview panel");
        assert_eq!(
            panel.get("entry").and_then(Value::as_str),
            Some("ui/index.html")
        );
        assert!(capabilities(&manifest.contributions).contains(&"preview".to_string()));
    }

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

    #[test]
    fn panel_document_injects_the_plugin_csp_into_head() {
        let html = panel_document(
            "<!doctype html><html><head><title>Panel</title></head><body></body></html>",
        );
        assert!(html.contains(PANEL_CSP));
        let head = html.find("<head>").unwrap();
        let policy = html.find(PANEL_CSP).unwrap();
        let title = html.find("<title>").unwrap();
        assert!(
            head < policy && policy < title,
            "CSP must precede the bundle"
        );
        assert_eq!(html.matches(PANEL_CSP).count(), 1);
    }

    #[test]
    fn panel_document_prepends_the_csp_when_there_is_no_head() {
        let html = panel_document("<body>fragment</body>");
        assert!(html.starts_with(PANEL_CSP));
        assert!(html.ends_with("<body>fragment</body>"));
    }
}
