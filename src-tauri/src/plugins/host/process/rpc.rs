//! Plugin JSON-RPC transport, concurrent reverse-request dispatch and
//! operation/task scoped `host.*` method handling.
use super::spawn::{host_exec, run_command};
use crate::plugin_sdk;
use serde_json::{json, Value};
use std::{
    collections::{BTreeSet, HashMap, VecDeque},
    fs,
    io::{BufReader, Read, Write},
    path::{Component, Path},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::Duration,
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_TASKS: usize = 32;
const MAX_TASK_HISTORY: usize = 64;

/// A plugin call failure. Business errors (a valid JSON-RPC error reply) must
/// not terminate the resident process; transport and protocol failures must.
#[derive(Debug)]
pub(super) struct RpcFailure {
    pub(super) message: String,
    pub(super) fatal: bool,
}

impl RpcFailure {
    fn business(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            fatal: false,
        }
    }

    fn fatal(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            fatal: true,
        }
    }
}

#[derive(Clone)]
struct Operation {
    owner: String,
    scope: Option<crate::plugin_fs_access::Scope>,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginTaskInfo {
    pub task_id: String,
    pub plugin_id: String,
    pub label: String,
    pub status: String,
    pub done: u64,
    pub total: Option<u64>,
    pub result: Value,
    pub error: Option<String>,
    pub internal: bool,
}

#[derive(Clone)]
struct TaskEntry {
    owner: String,
    scope: Option<crate::plugin_fs_access::Scope>,
    info: PluginTaskInfo,
}

struct Actor {
    owner: String,
    scope: Option<crate::plugin_fs_access::Scope>,
}

pub(super) struct Running {
    plugin_id: String,
    base_owner: String,
    declared: BTreeSet<String>,
    child: Mutex<crate::plugin_sandbox::SandboxChild>,
    stdin: Mutex<Box<dyn Write + Send>>,
    pending: Mutex<HashMap<u64, mpsc::Sender<Result<Value, RpcFailure>>>>,
    operations: Mutex<HashMap<u64, Operation>>,
    tasks: Mutex<HashMap<String, TaskEntry>>,
    history: Mutex<VecDeque<PluginTaskInfo>>,
    next_id: AtomicU64,
    initialized: AtomicBool,
    init_lock: Mutex<()>,
    closed: AtomicBool,
}

impl Running {
    pub(super) fn start(
        plugin_id: String,
        base_owner: String,
        declared: BTreeSet<String>,
        child: crate::plugin_sandbox::SandboxChild,
        stdin: Box<dyn Write + Send>,
        stdout: BufReader<Box<dyn Read + Send>>,
    ) -> Arc<Self> {
        let running = Arc::new(Self {
            plugin_id,
            base_owner,
            declared,
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            pending: Mutex::new(HashMap::new()),
            operations: Mutex::new(HashMap::new()),
            tasks: Mutex::new(HashMap::new()),
            history: Mutex::new(VecDeque::new()),
            next_id: AtomicU64::new(0),
            initialized: AtomicBool::new(false),
            init_lock: Mutex::new(()),
            closed: AtomicBool::new(false),
        });
        let reader_running = Arc::clone(&running);
        thread::Builder::new()
            .name("fiex-plugin-reader".into())
            .spawn(move || reader_running.read_loop(stdout))
            .expect("plugin reader thread");
        running
    }

    fn read_loop(self: Arc<Self>, mut stdout: BufReader<Box<dyn Read + Send>>) {
        loop {
            match plugin_sdk::read_frame(&mut stdout) {
                Ok(Some(message)) => self.route(message),
                Ok(None) => {
                    self.fail("plugin closed its control channel");
                    break;
                }
                Err(error) => {
                    self.fail(&format!("plugin protocol error: {error}"));
                    break;
                }
            }
        }
    }

    fn route(self: &Arc<Self>, message: Value) {
        if let Some(method) = message.get("method").and_then(Value::as_str) {
            let Some(id) = message.get("id").cloned() else {
                // Notifications currently have no host-side work.
                return;
            };
            let method = method.to_string();
            let params = message.get("params").cloned().unwrap_or(Value::Null);
            let running = Arc::clone(self);
            let reply_id = id.clone();
            let spawned = thread::Builder::new()
                .name("fiex-plugin-host-call".into())
                .spawn(move || {
                    let result = running.host_method(&method, &params);
                    let reply = match result {
                        Ok(value) => json!({"jsonrpc":"2.0","id":reply_id,"result":value}),
                        Err(message) => {
                            json!({"jsonrpc":"2.0","id":reply_id,"error":{"code":-32001,"message":message}})
                        }
                    };
                    let _ = running.send_frame(&reply);
                });
            if spawned.is_err() {
                let reply = json!({"jsonrpc":"2.0","id":id,
                    "error":{"code":-32001,"message":"host worker unavailable"}});
                let _ = self.send_frame(&reply);
            }
            return;
        }
        let Some(id) = message.get("id").and_then(Value::as_u64) else {
            return;
        };
        let result = if let Some(error) = message.get("error") {
            Err(RpcFailure::business(format!("plugin error: {error}")))
        } else {
            message
                .get("result")
                .cloned()
                .ok_or_else(|| RpcFailure::fatal("plugin response missing result"))
        };
        if let Ok(mut pending) = self.pending.lock() {
            if let Some(sender) = pending.remove(&id) {
                let _ = sender.send(result);
            }
        }
    }

    fn send_frame(&self, message: &Value) -> Result<(), String> {
        let mut writer = self
            .stdin
            .lock()
            .map_err(|_| "plugin input lock poisoned")?;
        plugin_sdk::write_frame(&mut *writer, message).map_err(|error| error.to_string())
    }

    fn fail(&self, message: &str) {
        self.closed.store(true, Ordering::Release);
        let diagnostic = self.exit_diagnostic();
        let message = format!("{message} ({diagnostic})");
        if let Ok(mut pending) = self.pending.lock() {
            for (_, sender) in pending.drain() {
                let _ = sender.send(Err(RpcFailure::fatal(message.clone())));
            }
        }
    }

    pub(super) fn stop(&self) {
        self.fail("plugin process stopped");
        let operations: Vec<Operation> = self
            .operations
            .lock()
            .map(|mut operations| operations.drain().map(|(_, value)| value).collect())
            .unwrap_or_default();
        for operation in operations {
            crate::plugin_fs_access::cleanup(&operation.owner);
            crate::plugin_ipc::cleanup(&operation.owner);
            crate::plugin_net::cleanup(&operation.owner);
        }
        let tasks: Vec<TaskEntry> = self
            .tasks
            .lock()
            .map(|mut tasks| tasks.drain().map(|(_, value)| value).collect())
            .unwrap_or_default();
        for task in tasks {
            let mut info = task.info;
            info.status = "cancelled".into();
            info.error = Some("插件进程已停止".into());
            self.emit_task(&info);
            crate::plugin_fs_access::cleanup(&task.owner);
            crate::plugin_ipc::cleanup(&task.owner);
            crate::plugin_net::cleanup(&task.owner);
        }
    }

    fn emit_task(&self, info: &PluginTaskInfo) {
        crate::plugin_task_events::emit(crate::plugin_task_events::PluginTaskEvent {
            task_id: info.task_id.clone(),
            plugin_id: info.plugin_id.clone(),
            label: info.label.clone(),
            status: info.status.clone(),
            done: info.done,
            total: info.total,
            result: info.result.clone(),
            error: info.error.clone(),
            internal: info.internal,
        });
    }

    pub(super) fn task(&self, id: &str) -> Option<PluginTaskInfo> {
        if let Some(info) = self
            .tasks
            .lock()
            .ok()
            .and_then(|tasks| tasks.get(id).map(|task| task.info.clone()))
        {
            return Some(info);
        }
        self.history
            .lock()
            .ok()
            .and_then(|history| history.iter().find(|info| info.task_id == id).cloned())
    }

    pub(super) fn cancel_task(&self, id: &str) -> Result<(), String> {
        let info = {
            let mut tasks = self.tasks.lock().map_err(|_| "task registry unavailable")?;
            let entry = tasks.get_mut(id).ok_or("background task not found")?;
            if !matches!(entry.info.status.as_str(), "running" | "cancelling") {
                return Err("background task has already finished".into());
            }
            entry.info.status = "cancelling".into();
            entry.info.clone()
        };
        self.emit_task(&info);
        if let Err(error) = self.send_frame(&json!({
            "jsonrpc": "2.0",
            "method": "plugin.cancel",
            "params": { "taskId": info.task_id },
        })) {
            if let Some(mut entry) = self
                .tasks
                .lock()
                .ok()
                .and_then(|mut tasks| tasks.remove(id))
            {
                entry.info.status = "failed".into();
                entry.info.error = Some("插件进程已断开".into());
                crate::plugin_fs_access::cleanup(&entry.owner);
                crate::plugin_ipc::cleanup(&entry.owner);
                crate::plugin_net::cleanup(&entry.owner);
                self.emit_task(&entry.info);
                self.remember_task(entry.info);
            }
            return Err(error);
        }
        Ok(())
    }

    fn remember_task(&self, info: PluginTaskInfo) {
        if let Ok(mut history) = self.history.lock() {
            history.push_back(info);
            while history.len() > MAX_TASK_HISTORY {
                history.pop_front();
            }
        }
    }

    pub(super) fn exit_diagnostic(&self) -> String {
        self.child
            .lock()
            .map(|mut child| child.exit_diagnostic())
            .unwrap_or_else(|_| "plugin process state unavailable".into())
    }

    fn operation_actor(&self, p: &Value) -> Result<Operation, String> {
        let authorization = p
            .get("authorization")
            .ok_or("missing operation authorization")?;
        let id = authorization
            .get("operationId")
            .and_then(Value::as_u64)
            .ok_or("missing operation authorization")?;
        self.operations
            .lock()
            .map_err(|_| "operation registry unavailable")?
            .get(&id)
            .cloned()
            .ok_or_else(|| "operation authorization expired".into())
    }

    fn actor(&self, p: &Value) -> Result<Actor, String> {
        let authorization = p.get("authorization").ok_or("missing authorization")?;
        if let Some(task) = authorization.get("taskId").and_then(Value::as_str) {
            let entry = self
                .tasks
                .lock()
                .map_err(|_| "task registry unavailable")?
                .get(task)
                .filter(|entry| matches!(entry.info.status.as_str(), "running" | "cancelling"))
                .cloned()
                .ok_or_else(|| "task authorization expired".to_string())?;
            return Ok(Actor {
                owner: entry.owner,
                scope: entry.scope,
            });
        }
        let id = authorization
            .get("operationId")
            .and_then(Value::as_u64)
            .ok_or("missing authorization")?;
        let operation = self
            .operations
            .lock()
            .map_err(|_| "operation registry unavailable")?
            .get(&id)
            .cloned()
            .ok_or_else(|| "operation authorization expired".to_string())?;
        Ok(Actor {
            owner: operation.owner,
            scope: operation.scope,
        })
    }

    /// All host.* requests are authenticated by the Running process identity,
    /// never by a pluginId supplied in the message. The operation or task
    /// session selects this particular request's authorization scope.
    fn host_method(&self, method: &str, p: &Value) -> Result<Value, String> {
        let arg = |key: &str| {
            p.get(key)
                .and_then(Value::as_str)
                .ok_or_else(|| format!("missing {key}"))
        };
        match method {
            "host.task.start" => {
                let operation = self.operation_actor(p)?;
                let task_id = arg("taskId")?;
                if task_id.is_empty()
                    || task_id.len() > 128
                    || !task_id
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
                {
                    return Err("invalid task id".into());
                }
                let label = p
                    .get("label")
                    .and_then(Value::as_str)
                    .filter(|label| !label.is_empty())
                    .unwrap_or("插件后台任务");
                let mut tasks = self.tasks.lock().map_err(|_| "task registry unavailable")?;
                if tasks.len() >= MAX_TASKS {
                    return Err("too many background tasks".into());
                }
                if tasks.contains_key(task_id) {
                    return Err("task id already registered".into());
                }
                let info = PluginTaskInfo {
                    task_id: task_id.to_string(),
                    plugin_id: self.plugin_id.clone(),
                    label: label.to_string(),
                    status: "running".into(),
                    done: 0,
                    total: None,
                    result: Value::Null,
                    error: None,
                    internal: p.get("internal").and_then(Value::as_bool).unwrap_or(false),
                };
                tasks.insert(
                    task_id.to_string(),
                    TaskEntry {
                        owner: format!("{}:task:{task_id}", self.base_owner),
                        scope: operation.scope,
                        info: info.clone(),
                    },
                );
                drop(tasks);
                self.emit_task(&info);
                Ok(Value::Null)
            }
            "host.task.finish" => {
                let authorization = p.get("authorization").ok_or("missing task authorization")?;
                let task_id = authorization
                    .get("taskId")
                    .and_then(Value::as_str)
                    .ok_or("missing task authorization")?;
                let finished = self
                    .tasks
                    .lock()
                    .map_err(|_| "task registry unavailable")?
                    .remove(task_id)
                    .map(|mut entry| {
                        entry.info.status = match p.get("status").and_then(Value::as_str) {
                            Some("completed") => "completed",
                            Some("cancelled") => "cancelled",
                            _ => "failed",
                        }
                        .to_string();
                        entry.info.result = p.get("result").cloned().unwrap_or(Value::Null);
                        entry.info.error =
                            p.get("error").and_then(Value::as_str).map(str::to_owned);
                        entry
                    });
                let Some(entry) = finished else {
                    return Ok(Value::Null);
                };
                crate::plugin_fs_access::cleanup(&entry.owner);
                crate::plugin_ipc::cleanup(&entry.owner);
                crate::plugin_net::cleanup(&entry.owner);
                self.emit_task(&entry.info);
                self.remember_task(entry.info);
                Ok(Value::Null)
            }
            "host.task.progress" => {
                let authorization = p.get("authorization").ok_or("missing task authorization")?;
                let task_id = authorization
                    .get("taskId")
                    .and_then(Value::as_str)
                    .ok_or("missing task authorization")?;
                let info = {
                    let mut tasks = self.tasks.lock().map_err(|_| "task registry unavailable")?;
                    let Some(entry) = tasks.get_mut(task_id) else {
                        return Err("task authorization expired".into());
                    };
                    if !matches!(entry.info.status.as_str(), "running" | "cancelling") {
                        return Ok(Value::Null);
                    }
                    entry.info.done = p.get("done").and_then(Value::as_u64).unwrap_or(0);
                    entry.info.total = p.get("total").and_then(Value::as_u64);
                    entry.info.clone()
                };
                self.emit_task(&info);
                Ok(Value::Null)
            }
            "host.task.cancel" => {
                let authorization = p.get("authorization").ok_or("missing task authorization")?;
                let task_id = authorization
                    .get("taskId")
                    .and_then(Value::as_str)
                    .ok_or("missing task authorization")?;
                if !self
                    .tasks
                    .lock()
                    .map_err(|_| "task registry unavailable")?
                    .contains_key(task_id)
                {
                    return Err("task authorization expired".into());
                }
                Ok(Value::Null)
            }
            _ => self.authorized_host_method(method, p),
        }
    }

    fn authorized_host_method(&self, method: &str, p: &Value) -> Result<Value, String> {
        let policy =
            crate::plugin_permission_policy::PermissionPolicy::new(&self.plugin_id, &self.declared);
        let permitted = |name: &str| policy.allowed_name(name);
        let mut actor = self.actor(p)?;
        if let Some(scope) = &actor.scope {
            scope.authorize_method(method)?;
        }
        let mut remote_params;
        let p = if method.starts_with("host.fs.") {
            let remote = actor
                .scope
                .as_ref()
                .map(|scope| scope.remote_target(p))
                .transpose()?
                .flatten();
            if let Some(resource) = remote {
                if method == "host.fs.stat" {
                    if !permitted("metadata") {
                        return Err("metadata permission not granted".into());
                    }
                    let mut value =
                        serde_json::to_value(resource.metadata()).map_err(|e| e.to_string())?;
                    value["path"] = json!(resource.display);
                    value["isDir"] = json!(resource.is_dir);
                    value["readonly"] = json!(resource.read_only);
                    value["symlink"] = json!(false);
                    for key in ["size", "modified"] {
                        if value[key].is_null() {
                            value[key] = json!(0);
                        }
                    }
                    return Ok(value);
                }
                let capability = match method {
                    "host.fs.openRead" | "host.fs.read" | "host.fs.readChunk" => "files.read",
                    "host.fs.list" => "folders.read",
                    "host.fs.beginWrite" => "files.write",
                    "host.fs.remove" => {
                        if resource.is_dir {
                            "folders.delete"
                        } else {
                            "files.delete"
                        }
                    }
                    "host.fs.rename" => {
                        if resource.is_dir {
                            "folders.rename"
                        } else {
                            "files.rename"
                        }
                    }
                    "host.fs.mkdir" => "folders.create",
                    _ => return Err("unsupported remote resource operation".into()),
                };
                if !permitted(capability) {
                    return Err(format!("permission not granted: {capability}"));
                }
                if method == "host.fs.list" {
                    let entries = if crate::remote::is_remote(&resource.display) {
                        futures::executor::block_on(crate::remote::list_listing(
                            &resource.display,
                            true,
                            &[],
                        ))?
                        .ok_or("remote directory unavailable")?
                        .entries
                    } else {
                        crate::plugin_vfs::list(&resource.display, true, &[])?.entries
                    };
                    let values=entries.into_iter().map(|e|json!({"path":e.path,"name":e.name,"isDir":e.is_dir,"size":e.size,"modified":e.modified,"extension":e.extension,"mime":e.mime,"readonly":e.readonly,"symlink":e.is_symlink})).collect::<Vec<_>>();
                    return Ok(json!(values));
                }
                let local = resource.materialize()?;
                let directory = local.parent().ok_or("remote target has no parent")?;
                let mut scope = crate::plugin_fs_access::Scope::selected(
                    directory,
                    std::slice::from_ref(&local),
                )?;
                let target = json!({"ref":p.get("ref").cloned().unwrap_or_else(||json!(resource.reference)),"metadata":resource.metadata()});
                scope.bind_resource(&target, Some(&local))?;
                actor.scope = Some(scope);
                remote_params = p.clone();
                remote_params["path"] = json!(local.to_string_lossy());
                &remote_params
            } else {
                p
            }
        } else {
            p
        };
        let mut resolved;
        let p = if let Some(reference) = p.get("ref") {
            let scope = actor.scope.as_ref().ok_or("no operation scope")?;
            if method == "host.fs.stat" {
                if !permitted("metadata") {
                    return Err("metadata permission not granted".into());
                }
                if let Some(metadata) = scope.metadata_reference(reference)? {
                    return Ok(metadata.clone());
                }
            }
            resolved = p.clone();
            resolved["path"] = json!(scope.resolve_reference(reference)?);
            &resolved
        } else if method.starts_with("host.fs.") {
            if let Some(path) = p
                .get("path")
                .and_then(Value::as_str)
                .map(|path| {
                    actor
                        .scope
                        .as_ref()
                        .map(|scope| scope.resource_alias(path))
                        .transpose()
                })
                .transpose()?
                .flatten()
                .flatten()
            {
                resolved = p.clone();
                resolved["path"] = json!(path);
                &resolved
            } else {
                p
            }
        } else {
            p
        };
        let arg = |key: &str| {
            p.get(key)
                .and_then(Value::as_str)
                .ok_or_else(|| format!("missing {key}"))
        };
        let owner = &actor.owner;
        let root = || {
            actor
                .scope
                .as_ref()
                .ok_or_else(|| "no task directory".to_string())
        };
        let scope = || arg("scope");
        let key = || arg("key");
        match method {
            "host.vfs.openView" => crate::mounts::open_view(&self.plugin_id, p, root()?),
            "host.net.http.open" if permitted("network") => {
                crate::plugin_net::http_open(owner, &policy, p)
            }
            "host.net.http.read" if permitted("network") => crate::plugin_net::http_read(owner, p),
            "host.net.http.close" => {
                crate::plugin_net::http_close(owner, p)?;
                Ok(Value::Null)
            }
            "host.net.http" if permitted("network") => crate::plugin_net::http(owner, &policy, p),
            #[cfg(feature = "desktop")]
            "host.terminal.output" => {
                crate::desktop::ssh::plugin_terminal_output(owner, p)?;
                Ok(Value::Null)
            }
            #[cfg(feature = "desktop")]
            "host.terminal.input" => crate::desktop::ssh::plugin_terminal_input(owner, p),
            #[cfg(feature = "desktop")]
            "host.terminal.size" => crate::desktop::ssh::plugin_terminal_size(owner, p),
            #[cfg(feature = "desktop")]
            "host.terminal.closed" => crate::desktop::ssh::plugin_terminal_closed(owner, p),
            #[cfg(feature = "desktop")]
            "host.terminal.end" => {
                crate::desktop::ssh::plugin_terminal_end(owner, p)?;
                Ok(Value::Null)
            }
            #[cfg(not(feature = "desktop"))]
            "host.terminal.output"
            | "host.terminal.input"
            | "host.terminal.size"
            | "host.terminal.closed"
            | "host.terminal.end" => Err("终端仅在桌面版可用".into()),
            "host.net.tcp.open" if permitted("network") => {
                crate::plugin_net::tcp_open(owner, &policy, p)
            }
            "host.net.tcp.readBytes" if permitted("network") => {
                crate::plugin_net::tcp_read_bytes(owner, p)
            }
            "host.net.tcp.writeBytes" if permitted("network") => {
                crate::plugin_net::tcp_write_bytes(owner, p)
            }
            "host.net.tcp.read" if permitted("network") => crate::plugin_net::tcp_read(owner, p),
            "host.net.tcp.write" if permitted("network") => crate::plugin_net::tcp_write(owner, p),
            "host.net.tcp.shutdown" if permitted("network") => {
                crate::plugin_net::tcp_shutdown(owner, p)?;
                Ok(Value::Null)
            }
            "host.net.tcp.close" if permitted("network") => {
                crate::plugin_net::tcp_close(owner, p)?;
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
                    .map_err(|error| error.to_string())?;
                if bytes.len() > 1024 * 1024 {
                    return Err("preview too large".into());
                }
                let directory = dirs::cache_dir()
                    .ok_or("cache directory unavailable")?
                    .join("fiex")
                    .join("plugin-previews")
                    .join(&self.plugin_id);
                fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
                let path = directory.join(name);
                fs::write(&path, bytes).map_err(|error| error.to_string())?;
                Ok(json!(path.to_string_lossy()))
            }
            "host.buffer.create" => {
                let size = p
                    .get("size")
                    .and_then(Value::as_u64)
                    .ok_or("missing size")? as usize;
                let writable = p.get("writable").and_then(Value::as_bool).unwrap_or(false);
                serde_json::to_value(crate::plugin_ipc::create(owner, size, writable)?)
                    .map_err(|error| error.to_string())
            }
            "host.buffer.map" => {
                let id = p.get("id").and_then(Value::as_str).ok_or("missing id")?;
                serde_json::to_value(crate::plugin_ipc::map(owner, id)?)
                    .map_err(|error| error.to_string())
            }
            "host.fs.readChunk" if permitted("files.read") => crate::plugin_fs_access::read_chunk(
                root()?,
                arg("path")?,
                p.get("offset").and_then(Value::as_u64).unwrap_or(0),
                p.get("length").and_then(Value::as_u64).unwrap_or(262144) as usize,
            ),
            "host.fs.openRead" if permitted("files.read") => {
                crate::plugin_fs_access::open_read(owner, root()?, arg("path")?)
            }
            "host.fs.readOpen" if permitted("files.read") => crate::plugin_fs_access::read_open(
                owner,
                arg("handle")?,
                p.get("offset")
                    .and_then(Value::as_u64)
                    .ok_or("missing offset")?,
                p.get("length")
                    .and_then(Value::as_u64)
                    .ok_or("missing length")?
                    .try_into()
                    .map_err(|_| "invalid length")?,
                p.get("buffer").and_then(Value::as_str),
            ),
            "host.fs.closeRead" => {
                crate::plugin_fs_access::close_read(owner, arg("handle")?)?;
                Ok(Value::Null)
            }
            "host.fs.mkdir" if permitted("folders.create") => {
                crate::plugin_fs_access::mkdir(root()?, arg("path")?)
            }
            "host.fs.beginWrite" => {
                let create = p.get("create").and_then(Value::as_bool).unwrap_or(false);
                if !permitted(if create {
                    "files.create"
                } else {
                    "files.write"
                }) {
                    return Err("file operation permission denied".into());
                }
                Ok(json!(crate::plugin_fs_access::begin_write(
                    owner,
                    root()?,
                    arg("path")?,
                    create
                )?))
            }
            "host.fs.seekWrite" => {
                if !permitted(crate::plugin_fs_access::writer_permission(
                    owner,
                    arg("handle")?,
                )?) {
                    return Err("file operation permission denied".into());
                }
                Ok(json!(crate::plugin_fs_access::seek_write(
                    owner,
                    arg("handle")?,
                    arg("kind")?,
                    p.get("offset")
                        .and_then(Value::as_i64)
                        .ok_or("missing seek offset")?
                )?))
            }
            "host.fs.writeChunk" => {
                if !permitted(crate::plugin_fs_access::writer_permission(
                    owner,
                    arg("handle")?,
                )?) {
                    return Err("file operation permission denied".into());
                }
                crate::plugin_fs_access::write_chunk(owner, arg("handle")?, arg("base64")?)?;
                Ok(Value::Null)
            }
            "host.fs.writeBuffer" => {
                if !permitted(crate::plugin_fs_access::writer_permission(
                    owner,
                    arg("handle")?,
                )?) {
                    return Err("file operation permission denied".into());
                }
                crate::plugin_fs_access::write_buffer(
                    owner,
                    arg("handle")?,
                    arg("buffer")?,
                    p.get("size")
                        .and_then(Value::as_u64)
                        .ok_or("missing size")?
                        .try_into()
                        .map_err(|_| "invalid size")?,
                )?;
                Ok(Value::Null)
            }
            "host.fs.commit" => {
                if !permitted(crate::plugin_fs_access::writer_permission(
                    owner,
                    arg("handle")?,
                )?) {
                    return Err("file operation permission denied".into());
                }
                crate::plugin_fs_access::commit(owner, arg("handle")?)
            }
            "host.fs.abort" => {
                crate::plugin_fs_access::abort(owner, arg("handle")?)?;
                Ok(Value::Null)
            }
            "host.fs.list" if permitted("folders.read") => {
                let path = p.get("path").and_then(Value::as_str).unwrap_or(".");
                crate::plugin_fs_access::list(root()?, path)
            }
            "host.fs.stat" if permitted("metadata") => {
                let path = p
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or("missing path")?;
                crate::plugin_fs_access::stat(root()?, path)
            }
            "host.fs.statBatch" if permitted("metadata") => {
                let paths = p
                    .get("paths")
                    .and_then(Value::as_array)
                    .ok_or("missing paths")?;
                crate::plugin_fs_access::stat_batch(root()?, paths)
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
                crate::plugin_fs_access::read(owner, root()?, path, offset, length)
            }
            "host.fs.create" if permitted("files.create") => {
                let path = p
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or("missing path")?;
                crate::plugin_fs_access::create(root()?, path)
            }
            "host.fs.write" => {
                let path = p
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or("missing path")?;
                let buffer = p
                    .get("buffer")
                    .and_then(Value::as_str)
                    .ok_or("missing buffer")?;
                let mode = p.get("mode").and_then(Value::as_str).unwrap_or("truncate");
                if !permitted(if mode == "create" {
                    "files.create"
                } else {
                    "files.write"
                }) {
                    return Err("file operation permission denied".into());
                }
                crate::plugin_fs_access::write(owner, root()?, path, buffer, mode)
            }
            "host.fs.remove" => {
                let path = p
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or("missing path")?;
                let required = if crate::plugin_fs_access::is_directory(root()?, path)? {
                    "folders.delete"
                } else {
                    "files.delete"
                };
                if !permitted(required) {
                    return Err("file operation permission denied".into());
                }
                crate::plugin_fs_access::remove(root()?, path)?;
                Ok(Value::Null)
            }
            "host.fs.rename" => {
                let from = p
                    .get("from")
                    .and_then(Value::as_str)
                    .ok_or("missing from")?;
                let to = p.get("to").and_then(Value::as_str).ok_or("missing to")?;
                let required = if crate::plugin_fs_access::is_directory(root()?, from)? {
                    "folders.rename"
                } else {
                    "files.rename"
                };
                if !permitted(required) {
                    return Err("file operation permission denied".into());
                }
                crate::plugin_fs_access::rename(root()?, from, to)?;
                Ok(Value::Null)
            }
            "host.buffer.release" => {
                let id = p.get("id").and_then(Value::as_str).ok_or("missing id")?;
                crate::plugin_ipc::release(owner, id)?;
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
                actor.scope.as_ref().map(|scope| scope.path()),
            ),
            _ => Err("host method not available or permission denied".into()),
        }
    }

    fn ensure_initialized(&self) -> Result<(), RpcFailure> {
        if self.initialized.load(Ordering::Acquire) {
            return Ok(());
        }
        let _guard = self
            .init_lock
            .lock()
            .map_err(|_| RpcFailure::fatal("plugin initialization lock poisoned"))?;
        if self.initialized.load(Ordering::Acquire) {
            return Ok(());
        }
        let result = self
            .rpc(
                "plugin.initialize",
                json!({"protocolVersion": plugin_sdk::PROTOCOL_VERSION}),
                None,
            )
            .map_err(|failure| RpcFailure::fatal(failure.message))?;
        if result.get("pluginId").and_then(Value::as_str) != Some(self.plugin_id.as_str()) {
            return Err(RpcFailure::fatal("plugin identity mismatch"));
        }
        self.initialized.store(true, Ordering::Release);
        Ok(())
    }

    pub(super) fn rpc(
        &self,
        method: &str,
        params: Value,
        scope: Option<crate::plugin_fs_access::Scope>,
    ) -> Result<Value, RpcFailure> {
        if self.closed.load(Ordering::Acquire) {
            return Err(RpcFailure::fatal("plugin process is not running"));
        }
        if method != "plugin.initialize" {
            self.ensure_initialized()?;
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let owner = format!("{}:op:{id}", self.base_owner);
        self.operations
            .lock()
            .map_err(|_| RpcFailure::fatal("operation registry unavailable"))?
            .insert(
                id,
                Operation {
                    owner: owner.clone(),
                    scope,
                },
            );
        let (sender, receiver) = mpsc::channel();
        self.pending
            .lock()
            .map_err(|_| RpcFailure::fatal("response registry unavailable"))?
            .insert(id, sender);
        let sent = self.send_frame(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }));
        if let Err(error) = sent {
            if let Ok(mut pending) = self.pending.lock() {
                pending.remove(&id);
            }
            self.finish_operation(id);
            return Err(RpcFailure::fatal(error));
        }
        let result = match receiver.recv_timeout(if method == "fs.writeFile" {
            Duration::from_secs(86_520)
        } else {
            REQUEST_TIMEOUT
        }) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                Err(RpcFailure::fatal("plugin request deadline exceeded"))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(RpcFailure::fatal("plugin process disconnected"))
            }
        };
        if let Ok(mut pending) = self.pending.lock() {
            pending.remove(&id);
        }
        self.finish_operation(id);
        result
    }

    fn finish_operation(&self, id: u64) {
        let operation = self
            .operations
            .lock()
            .ok()
            .and_then(|mut operations| operations.remove(&id));
        if let Some(operation) = operation {
            crate::plugin_fs_access::cleanup(&operation.owner);
            crate::plugin_ipc::cleanup(&operation.owner);
            crate::plugin_net::cleanup(&operation.owner);
        }
    }
}
