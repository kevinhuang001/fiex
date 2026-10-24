//! Plugin child-process lifecycle, host method dispatch and JSON-RPC transport.
use super::catalog::{discover, executable};
use super::manifest::{activation_requirements, declared};
use crate::plugin_sdk;
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io::{BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

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
    declared: &BTreeSet<String>,
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
    declared: &BTreeSet<String>,
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
    declared: BTreeSet<String>,
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
