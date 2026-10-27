//! FsResolver: the one Core-side path to a provider's `fs.*` methods.
//!
//! Connection mounts and standalone views differ only in the identity block
//! they send; method names, DTO validation, pagination, stream handling and
//! error mapping are shared. Callers hand in a [`Mount`] built from the mount
//! registry, so no host path ever reaches a plugin.
use crate::plugin_providers::FsOperation;
use serde_json::{json, Value};

const PAGE_LIMIT: usize = 500;
const MAX_PAGES: usize = 1000;
const MAX_ENTRIES: usize = 100_000;

/// Where a mount's plugin identity comes from.
#[derive(Clone, Debug)]
pub enum Source {
    /// Connection-backed mount: the provider gets its settings and secrets.
    Connection(Value),
    /// Standalone view: the provider gets its contribution and view context.
    View {
        contribution_id: String,
        view_context: Value,
    },
}

/// One resolved mount plus the scope its provider calls run under.
#[derive(Clone)]
pub struct Mount {
    pub mount_id: String,
    pub plugin_id: String,
    pub source: Source,
    pub scope: Option<crate::plugin_fs_access::Scope>,
}

impl Mount {
    pub fn view_scope(&self) -> Option<&crate::plugin_fs_access::Scope> {
        self.scope.as_ref()
    }

    fn invoke(&self, method: &str, mut params: Value) -> Result<Value, String> {
        let object = params
            .as_object_mut()
            .ok_or("file-system parameters must be an object")?;
        object.insert("mountId".into(), json!(self.mount_id));
        match &self.source {
            Source::Connection(config) => {
                object.insert("connection".into(), config.clone());
            }
            Source::View {
                contribution_id,
                view_context,
            } => {
                object.insert("contributionId".into(), json!(contribution_id));
                object.insert("viewContext".into(), view_context.clone());
            }
        }
        crate::plugin_host::global().invoke_fs(&self.plugin_id, method, params, self.scope.clone())
    }

    pub fn list(
        &self,
        node_id: &str,
        hidden: bool,
        ignores: &[String],
    ) -> Result<FsListing, String> {
        let mut cursor: Option<String> = None;
        let mut entries = Vec::new();
        let mut warnings = Vec::new();
        for page_index in 0..MAX_PAGES {
            let _ = page_index;
            let page = self.invoke(
                "fs.list",
                json!({
                    "nodeId": node_id,
                    "hidden": hidden,
                    "ignores": ignores,
                    "cursor": cursor,
                    "limit": PAGE_LIMIT,
                }),
            )?;
            let items = page
                .get("entries")
                .and_then(Value::as_array)
                .ok_or("provider listing is missing entries")?;
            if items.len() > PAGE_LIMIT {
                return Err("provider exceeded the page limit".into());
            }
            for item in items {
                entries.push(FsEntry::parse(item.clone())?);
                if entries.len() > MAX_ENTRIES {
                    return Err("provider returned too many entries".into());
                }
            }
            if let Some(values) = page.get("warnings").and_then(Value::as_array) {
                warnings.extend(values.iter().filter_map(Value::as_str).map(str::to_owned));
            }
            let next = page
                .get("nextCursor")
                .and_then(Value::as_str)
                .map(str::to_owned);
            match next {
                None => {
                    return Ok(FsListing {
                        entries,
                        warnings,
                        truncated: false,
                    })
                }
                Some(next) if Some(&next) == cursor.as_ref() => {
                    return Err("provider pagination cursor did not advance".into())
                }
                Some(next) => cursor = Some(next),
            }
        }
        Err("provider sent too many pages".into())
    }

    pub fn stat(&self, node_id: &str) -> Result<FsEntry, String> {
        let value = self.invoke("fs.stat", json!({"nodeId": node_id}))?;
        FsEntry::parse(value)
    }

    pub fn search(
        &self,
        node_id: &str,
        query: &str,
        hidden: bool,
        ignores: &[String],
        limit: usize,
    ) -> Result<(Vec<FsEntry>, bool), String> {
        let value = self.invoke(
            "fs.search",
            json!({
                "nodeId": node_id,
                "query": query,
                "hidden": hidden,
                "ignores": ignores,
                "limit": limit,
            }),
        )?;
        let items = value
            .get("entries")
            .and_then(Value::as_array)
            .ok_or("provider search is missing entries")?;
        if items.len() > limit {
            return Err("provider exceeded the search limit".into());
        }
        let entries = items
            .iter()
            .map(|item| FsEntry::parse(item.clone()))
            .collect::<Result<Vec<_>, _>>()?;
        let truncated = value
            .get("truncated")
            .and_then(Value::as_bool)
            .unwrap_or(entries.len() >= limit);
        Ok((entries, truncated))
    }

    pub fn mkdir(&self, parent_id: &str, name: &str) -> Result<FsEntry, String> {
        crate::mounts::segment(name)?;
        let value = self.invoke("fs.mkdir", json!({"parentId": parent_id, "name": name}))?;
        FsEntry::parse(value)
    }

    pub fn rename(
        &self,
        node_id: &str,
        name: &str,
        parent_id: Option<&str>,
    ) -> Result<FsEntry, String> {
        crate::mounts::segment(name)?;
        let value = self.invoke(
            "fs.rename",
            json!({"nodeId": node_id, "name": name, "parentId": parent_id}),
        )?;
        FsEntry::parse(value)
    }

    pub fn remove(&self, node_id: &str) -> Result<(), String> {
        self.invoke("fs.remove", json!({"nodeId": node_id}))?;
        Ok(())
    }

    pub fn open_read(&self, node_id: &str) -> Result<FsStream, String> {
        let value = self.invoke("fs.openRead", json!({"nodeId": node_id}))?;
        let stream_id = value
            .get("streamId")
            .and_then(Value::as_str)
            .ok_or("provider did not return a stream id")?
            .to_string();
        Ok(FsStream {
            mount: self.clone(),
            stream_id,
            size: value.get("size").and_then(Value::as_u64),
            closed: false,
        })
    }

    pub fn begin_write(&self, request: WriteRequest<'_>) -> Result<FsWriteStream, String> {
        if let Some(name) = request.name {
            crate::mounts::segment(name)?;
        }
        let value = self.invoke(
            "fs.beginWrite",
            json!({
                "nodeId": request.node_id,
                "parentId": request.parent_id,
                "name": request.name,
                "size": request.size,
                "ifRevision": request.if_revision,
            }),
        )?;
        let stream_id = value
            .get("streamId")
            .and_then(Value::as_str)
            .ok_or("provider did not return a write stream id")?
            .to_string();
        Ok(FsWriteStream {
            mount: self.clone(),
            stream_id,
            offset: 0,
            finished: false,
        })
    }

    /// Whether the provider declared an operation, for UI decisions.
    pub fn supports(&self, operation: FsOperation) -> bool {
        crate::plugin_providers::supports(&self.plugin_id, operation)
    }
}

/// The plugin that owns a Core mount URI. Works without the desktop feature,
/// so UI-affordance code can consult declared capabilities everywhere.
pub fn mount_plugin_id(path: &str) -> Result<String, String> {
    let (_, source) = crate::mounts::resolve(path)?;
    Ok(match source {
        crate::mounts::Source::Connection(connection) => connection.plugin_id,
        crate::mounts::Source::View(view) => view.descriptor.plugin_id,
    })
}

/// Resolve a Core mount URI (`fiex://mount/<id>/..`) to the provider that
/// owns it. Used by navigation and UI affordance code; protocol calls receive a
/// prepared [`Mount`] instead.
#[cfg(feature = "desktop")]
pub fn mount_for_path(path: &str) -> Result<Mount, String> {
    let (uri, source) = crate::mounts::resolve(path)?;
    match source {
        crate::mounts::Source::Connection(connection) => {
            crate::remote::ensure_provider_access(&connection)?;
            Ok(Mount {
                mount_id: uri.mount_id,
                plugin_id: connection.plugin_id.clone(),
                source: Source::Connection(crate::remote::provider_config(&connection)),
                scope: None,
            })
        }
        crate::mounts::Source::View(view) => Ok(Mount {
            mount_id: uri.mount_id,
            plugin_id: view.descriptor.plugin_id.clone(),
            source: Source::View {
                contribution_id: view.descriptor.contribution_id.clone(),
                view_context: view.descriptor.view_context.clone(),
            },
            scope: Some(view.scope.clone()),
        }),
    }
}

/// Parameters for a conditional write.
pub struct WriteRequest<'a> {
    pub node_id: Option<&'a str>,
    pub parent_id: Option<&'a str>,
    pub name: Option<&'a str>,
    pub size: Option<u64>,
    pub if_revision: Option<&'a str>,
}

/// Kind of node as reported by a provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FsKind {
    File,
    Directory,
    Symlink,
}

/// One provider entry after validation. Paths never appear on the wire; Core
/// binds `node_id` to its own mount-relative location.
#[derive(Clone, Debug)]
pub struct FsEntry {
    pub node_id: String,
    pub name: String,
    pub kind: FsKind,
    pub size: Option<u64>,
    pub modified: Option<i64>,
    pub extension: Option<String>,
    pub mime: Option<String>,
    pub revision: Option<String>,
    pub readonly: Option<bool>,
    pub link_target: Option<String>,
    pub allowed_ops: Option<Vec<FsOperation>>,
    pub relative_path: Option<String>,
}

impl FsEntry {
    pub fn parse(value: Value) -> Result<Self, String> {
        let node_id = value
            .get("nodeId")
            .and_then(Value::as_str)
            .ok_or("provider entry is missing nodeId")?
            .to_string();
        if node_id.len() > 16_384 || node_id.contains('\0') {
            return Err("provider returned an invalid node id".into());
        }
        let name = value
            .get("name")
            .and_then(Value::as_str)
            .ok_or("provider entry is missing name")?
            .to_string();
        crate::mounts::segment(&name)?;
        let kind = match value.get("kind").and_then(Value::as_str) {
            Some("directory") => FsKind::Directory,
            Some("symlink") => FsKind::Symlink,
            Some("file") | None => FsKind::File,
            Some(_) => return Err("provider returned an unknown node kind".into()),
        };
        let allowed_ops = value
            .get("allowedOps")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .filter_map(FsOperation::parse)
                    .collect()
            });
        Ok(Self {
            node_id,
            name,
            kind,
            size: value.get("size").and_then(Value::as_u64),
            modified: value.get("modified").and_then(Value::as_i64).or_else(|| {
                value
                    .get("modified")
                    .and_then(Value::as_u64)
                    .map(|v| v as i64)
            }),
            extension: value
                .get("extension")
                .and_then(Value::as_str)
                .map(str::to_owned),
            mime: value.get("mime").and_then(Value::as_str).map(str::to_owned),
            revision: value
                .get("revision")
                .and_then(Value::as_str)
                .map(str::to_owned),
            readonly: value.get("readonly").and_then(Value::as_bool),
            link_target: value
                .get("linkTarget")
                .and_then(Value::as_str)
                .map(str::to_owned),
            allowed_ops,
            relative_path: value
                .get("relativePath")
                .and_then(Value::as_str)
                .map(str::to_owned),
        })
    }

    pub fn is_dir(&self) -> bool {
        self.kind == FsKind::Directory
    }

    /// The node allows `operation` when it did not narrow the provider set.
    pub fn allows(&self, operation: FsOperation) -> bool {
        self.allowed_ops
            .as_ref()
            .is_none_or(|ops| ops.contains(&operation))
    }

    pub fn is_readonly(&self) -> bool {
        self.readonly.unwrap_or(false)
            || !(self.allows(FsOperation::Write)
                || self.allows(FsOperation::Mkdir)
                || self.allows(FsOperation::Rename)
                || self.allows(FsOperation::Remove))
    }
}

/// One validated listing page collection.
#[derive(Default)]
pub struct FsListing {
    pub entries: Vec<FsEntry>,
    pub warnings: Vec<String>,
    pub truncated: bool,
}

/// An open read stream. Dropping it releases the provider-side handle.
pub struct FsStream {
    mount: Mount,
    stream_id: String,
    size: Option<u64>,
    closed: bool,
}

impl FsStream {
    pub fn size(&self) -> Option<u64> {
        self.size
    }

    /// Read up to `max_bytes` starting at the absolute `offset`.
    pub fn read_at(&self, offset: u64, max_bytes: usize) -> Result<(Vec<u8>, bool), String> {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let value = self.mount.invoke(
            "fs.readStream",
            json!({
                "streamId": self.stream_id,
                "offset": offset,
                "maxBytes": max_bytes.clamp(1, 4 * 1024 * 1024),
            }),
        )?;
        let encoded = value
            .get("base64")
            .and_then(Value::as_str)
            .ok_or("provider stream reply is missing data")?;
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|_| "provider returned invalid base64 data".to_string())?;
        if bytes.len() > max_bytes {
            return Err("provider exceeded the requested chunk size".into());
        }
        let eof = value.get("eof").and_then(Value::as_bool).unwrap_or(false) || bytes.is_empty();
        Ok((bytes, eof))
    }

    pub fn close(mut self) {
        self.release();
    }

    fn release(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        let _ = self
            .mount
            .invoke("fs.closeRead", json!({"streamId": self.stream_id}));
    }
}

impl Drop for FsStream {
    fn drop(&mut self) {
        self.release();
    }
}

/// An open write stream. Content becomes visible only after `commit`.
pub struct FsWriteStream {
    mount: Mount,
    stream_id: String,
    offset: u64,
    finished: bool,
}

impl FsWriteStream {
    pub fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        use base64::{engine::general_purpose::STANDARD, Engine};
        if bytes.is_empty() {
            return Ok(());
        }
        let chunk = bytes.len().min(4 * 1024 * 1024);
        let value = self.mount.invoke(
            "fs.writeStream",
            json!({
                "streamId": self.stream_id,
                "offset": self.offset,
                "base64": STANDARD.encode(&bytes[..chunk]),
            }),
        )?;
        let written = value
            .get("written")
            .and_then(Value::as_u64)
            .unwrap_or(chunk as u64);
        self.offset += written.max(1);
        if written as usize != chunk {
            return Err("provider accepted only part of a chunk".into());
        }
        Ok(())
    }

    pub fn commit(mut self) -> Result<Option<FsEntry>, String> {
        self.finished = true;
        let value = self
            .mount
            .invoke("fs.commit", json!({"streamId": self.stream_id}))?;
        if value.is_null() || value.as_object().is_some_and(|object| object.is_empty()) {
            return Ok(None);
        }
        FsEntry::parse(value).map(Some)
    }

    pub fn abort(mut self) -> Result<(), String> {
        self.finished = true;
        self.mount
            .invoke("fs.abort", json!({"streamId": self.stream_id}))?;
        Ok(())
    }
}

impl Drop for FsWriteStream {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self
                .mount
                .invoke("fs.abort", json!({"streamId": self.stream_id}));
            self.finished = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(value: Value) -> Result<FsEntry, String> {
        FsEntry::parse(value)
    }

    #[test]
    fn entries_require_a_stable_node_id_and_a_safe_name() {
        let base = json!({"nodeId":"n1","name":"a.txt","kind":"file","size":4});
        assert_eq!(entry(base.clone()).unwrap().node_id, "n1");
        let mut missing = base.clone();
        missing.as_object_mut().unwrap().remove("nodeId");
        assert!(entry(missing).is_err());
        let mut traversal = base.clone();
        traversal["name"] = json!("../etc");
        assert!(entry(traversal).is_err());
        let mut nul = base;
        nul["nodeId"] = json!("n\0");
        assert!(entry(nul).is_err());
    }

    #[test]
    fn node_capabilities_narrow_the_provider() {
        let read_only = entry(json!({
            "nodeId":"n1","name":"a.txt","kind":"file","allowedOps":["read","stat"]
        }))
        .unwrap();
        assert!(read_only.allows(FsOperation::Read));
        assert!(!read_only.allows(FsOperation::Rename));
        assert!(read_only.is_readonly());
        let writable = entry(json!({
            "nodeId":"n2","name":"b.txt","kind":"file","allowedOps":["read","write","rename","remove"]
        }))
        .unwrap();
        assert!(!writable.is_readonly());
    }

    #[test]
    fn write_requests_validate_the_leaf_name() {
        let mount = Mount {
            mount_id: "m".into(),
            plugin_id: "org.example".into(),
            source: Source::Connection(Value::Null),
            scope: None,
        };
        let error = match mount.begin_write(WriteRequest {
            node_id: None,
            parent_id: Some("root"),
            name: Some("bad/name"),
            size: None,
            if_revision: None,
        }) {
            Err(error) => error,
            Ok(_) => panic!("expected a rejected name"),
        };
        assert!(error.contains("segment"));
    }
}
