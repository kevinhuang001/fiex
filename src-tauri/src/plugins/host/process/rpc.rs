//! Plugin JSON-RPC transport, request deadlines and `host.*` method dispatch.
use super::spawn::{host_exec, run_command};
use crate::plugin_sdk;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Component, Path};
use std::time::{Duration, Instant};

pub(super) struct Running {
    pub(super) plugin_id: String,
    pub(super) resource_owner: String,
    pub(super) declared: BTreeSet<String>,
    pub(super) child: crate::plugin_sandbox::SandboxChild,
    pub(super) stdin: Box<dyn Write + Send>,
    pub(super) stdout: std::sync::mpsc::Receiver<Result<Option<Value>, String>>,
    pub(super) next_id: u64,
    pub(super) task_root: Option<crate::plugin_fs_access::Scope>,
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
            "host.fs.openRead" if permitted("files.read") => {
                crate::plugin_fs_access::open_read(&self.resource_owner, root()?, arg("path")?)
            }
            "host.fs.readOpen" if permitted("files.read") => crate::plugin_fs_access::read_open(
                &self.resource_owner,
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
                crate::plugin_fs_access::close_read(&self.resource_owner, arg("handle")?)?;
                Ok(Value::Null)
            }
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
            "host.fs.writeBuffer" if permitted("files.write") => {
                crate::plugin_fs_access::write_buffer(
                    &self.resource_owner,
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
    pub(super) fn rpc(
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
