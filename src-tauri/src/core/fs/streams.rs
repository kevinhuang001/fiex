//! Actor-owned bounded stream handles. Registry locks are never held across
//! provider IPC; each stream serializes only its own operations.
use super::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};
struct Stream {
    resource: ResourceBroker,
    permission: &'static str,
    state: Mutex<Option<State>>,
}
enum State {
    Read(Box<dyn ReadHandle>),
    Write(Box<dyn WriteHandle>),
}
fn registry() -> &'static Mutex<HashMap<String, Arc<Stream>>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, Arc<Stream>>>> = OnceLock::new();
    REGISTRY.get_or_init(Default::default)
}
fn insert(
    resource: &ResourceBroker,
    state: State,
    permission: &'static str,
) -> Result<String, String> {
    let id = crate::plugins::mounts::new_id()?;
    let mut registry = registry()
        .lock()
        .map_err(|_| "stream registry unavailable")?;
    if registry
        .values()
        .filter(|s| s.resource.owner == resource.owner)
        .count()
        >= 32
    {
        return Err("too many open streams".into());
    }
    registry.insert(
        id.clone(),
        Arc::new(Stream {
            resource: resource.clone_binding(),
            permission,
            state: Mutex::new(Some(state)),
        }),
    );
    Ok(id)
}
pub(super) fn open_read(
    resource: &ResourceBroker,
    read: Box<dyn ReadHandle>,
) -> Result<Value, String> {
    let size = read.size();
    Ok(json!({"handle":insert(resource, State::Read(read), "files.read")?,"size":size}))
}
pub(super) fn begin_write(
    resource: &ResourceBroker,
    write: Box<dyn WriteHandle>,
    create: bool,
) -> Result<Value, String> {
    Ok(json!(insert(
        resource,
        State::Write(write),
        if create {
            "files.create"
        } else {
            "files.write"
        }
    )?))
}
fn get(owner: &str, id: &str) -> Result<Arc<Stream>, String> {
    registry()
        .lock()
        .map_err(|_| "stream registry unavailable")?
        .get(id)
        .filter(|s| s.resource.owner == owner)
        .cloned()
        .ok_or_else(|| "ScopeDenied: stream unavailable".into())
}
pub fn stream_permission(owner: &str, id: &str) -> Result<&'static str, String> {
    Ok(get(owner, id)?.permission)
}
pub fn stream_call(owner: &str, method: &str, params: &Value) -> Result<Value, String> {
    let id = params["handle"].as_str().ok_or("missing handle")?;
    let stream = get(owner, id)?;
    let mut state = stream.state.lock().map_err(|_| "stream unavailable")?;
    if matches!(method, "fs.closeRead" | "fs.abort") {
        let valid = matches!(
            (method, state.as_ref()),
            ("fs.closeRead", Some(State::Read(_))) | ("fs.abort", Some(State::Write(_)))
        );
        if !valid {
            return Err("invalid stream method".into());
        }
        state.take();
        registry()
            .lock()
            .map_err(|_| "stream registry unavailable")?
            .remove(id);
        return Ok(Value::Null);
    }
    stream.resource.scope.authorize_method(method)?;
    match state.as_mut().ok_or("stream closed")? {
        State::Read(read) if method == "fs.readOpen" => {
            stream.resource.require(FsOperation::Read, false)?;
            let offset = params["offset"].as_u64().ok_or("missing offset")?;
            let length: usize = params["length"]
                .as_u64()
                .ok_or("missing length")?
                .min(CHUNK as u64) as usize;
            let (bytes, eof) = read.read_at(offset, length)?;
            if bytes.len() > length {
                return Err("provider exceeded chunk limit".into());
            }
            if let Some(buffer) = params["buffer"].as_str() {
                crate::plugins::ipc::write_owned(owner, buffer, &bytes)?;
                Ok(json!({"size":bytes.len(),"eof":eof}))
            } else {
                Ok(json!({"base64":STANDARD.encode(&bytes),"size":bytes.len(),"eof":eof}))
            }
        }
        State::Write(write) => {
            stream
                .resource
                .require(FsOperation::Write, stream.permission == "files.create")?;
            match method {
                "fs.seekWrite" => Ok(json!(write.seek(
                    params["kind"].as_str().ok_or("missing seek kind")?,
                    params["offset"].as_i64().ok_or("missing offset")?
                )?)),
                "fs.writeChunk" | "fs.writeBuffer" => {
                    let bytes = if method == "fs.writeBuffer" {
                        let size = params["size"]
                            .as_u64()
                            .filter(|n| *n <= CHUNK as u64)
                            .ok_or("invalid chunk size")?
                            as usize;
                        let bytes = crate::plugins::ipc::read(
                            owner,
                            params["buffer"].as_str().ok_or("missing buffer")?,
                        )?;
                        bytes.get(..size).ok_or("buffer too small")?.to_vec()
                    } else {
                        let encoded = params["base64"]
                            .as_str()
                            .filter(|s| s.len() <= CHUNK.div_ceil(3) * 4)
                            .ok_or("invalid chunk")?;
                        STANDARD.decode(encoded).map_err(|e| e.to_string())?
                    };
                    if bytes.len() > CHUNK {
                        return Err("chunk too large".into());
                    }
                    write.write(&bytes)?;
                    Ok(Value::Null)
                }
                "fs.commit" => {
                    let Some(State::Write(write)) = state.take() else {
                        return Err("write stream closed".into());
                    };
                    registry()
                        .lock()
                        .map_err(|_| "stream registry unavailable")?
                        .remove(id);
                    let result = write.commit()?;
                    super::changes::committed(&stream.resource.path);
                    Ok(result)
                }
                _ => Err("invalid stream method".into()),
            }
        }
        _ => Err("invalid stream method".into()),
    }
}
pub fn cleanup(owner: &str) {
    let removed = if let Ok(mut registry) = registry().lock() {
        let ids: Vec<_> = registry
            .iter()
            .filter(|(_, s)| s.resource.owner == owner)
            .map(|(id, _)| id.clone())
            .collect();
        ids.into_iter()
            .filter_map(|id| registry.remove(&id))
            .collect::<Vec<_>>()
    } else {
        vec![]
    };
    // Dropping outside the registry lock permits provider close/abort IPC.
    drop(removed);
}

/// HTTP bodies retain the stream's actor binding and recheck the read capability
/// on each bounded read. The network policy is checked by the HTTP service.
pub fn http_body(owner: &str, handle: &str) -> Result<HttpBody, String> {
    let stream = get(owner, handle)?;
    if stream.permission != "files.read" {
        return Err("expected a read stream".into());
    }
    let state = stream.state.lock().map_err(|_| "stream unavailable")?;
    let Some(State::Read(read)) = state.as_ref() else {
        return Err("expected a read stream".into());
    };
    let size = read.size();
    Ok(HttpBody {
        owner: owner.into(),
        handle: handle.into(),
        offset: 0,
        size,
    })
}
pub struct HttpBody {
    owner: String,
    handle: String,
    offset: u64,
    size: u64,
}
impl HttpBody {
    pub fn size(&self) -> u64 {
        self.size
    }
    pub fn restart(&self) -> Result<Self, String> {
        http_body(&self.owner, &self.handle)
    }
}
impl std::io::Read for HttpBody {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let stream = get(&self.owner, &self.handle).map_err(std::io::Error::other)?;
        stream
            .resource
            .scope
            .authorize_method("fs.readOpen")
            .map_err(std::io::Error::other)?;
        let plugin_id = self
            .owner
            .split(':')
            .next()
            .ok_or_else(|| std::io::Error::other("invalid actor"))?;
        let plugin = crate::plugins::host::catalog()
            .into_iter()
            .find(|p| p.id == plugin_id && p.enabled && p.approved && p.backend_available)
            .ok_or_else(|| std::io::Error::other("plugin access revoked"))?;
        let declared = plugin.declared_permissions.into_iter().collect();
        crate::plugins::security::policy::PermissionPolicy::new(plugin_id, &declared)
            .require(crate::plugins::security::policy::Capability::FilesRead)
            .map_err(std::io::Error::other)?;
        stream
            .resource
            .require(FsOperation::Read, false)
            .map_err(std::io::Error::other)?;
        let mut state = stream
            .state
            .lock()
            .map_err(|_| std::io::Error::other("stream unavailable"))?;
        let Some(State::Read(read)) = state.as_mut() else {
            return Err(std::io::Error::other("read stream closed"));
        };
        let (bytes, _) = read
            .read_at(self.offset, output.len().min(CHUNK))
            .map_err(std::io::Error::other)?;
        if bytes.len() > output.len() {
            return Err(std::io::Error::other("provider exceeded read limit"));
        }
        output[..bytes.len()].copy_from_slice(&bytes);
        self.offset = self
            .offset
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| std::io::Error::other("read offset overflow"))?;
        Ok(bytes.len())
    }
}
