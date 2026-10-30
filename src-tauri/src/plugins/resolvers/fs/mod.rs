//! FsResolver: the one Core-side path to a provider's `vfs.*` methods.
//!
//! Connection mounts and standalone views differ only in the identity block
//! they send; method names, DTO validation, pagination, stream handling and
//! error mapping are shared. Callers hand in a [`Mount`] built from the mount
//! registry, so no host path ever reaches a plugin.
use crate::plugins::provider_registry::FsOperation;
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
    pub scope: Option<crate::plugins::resource::fs_access::Scope>,
}

impl Mount {
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
        crate::plugins::host::global().invoke_fs(
            &self.plugin_id,
            method,
            params,
            self.scope.clone(),
        )
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
        let mut cursors = std::collections::HashSet::new();
        for _ in 0..MAX_PAGES {
            let page = self.invoke(
                "vfs.list",
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
                Some(next) if !cursors.insert(next.clone()) => {
                    return Err("provider pagination cursor did not advance".into())
                }
                Some(next) => cursor = Some(next),
            }
        }
        Err("provider sent too many pages".into())
    }

    pub fn preflight(
        &self,
        request: &crate::plugin_sdk::filesystem::TransferPreflight,
    ) -> Result<(), String> {
        if !crate::plugins::provider_registry::get(&self.plugin_id)
            .is_some_and(|provider| provider.preflight)
        {
            return Ok(());
        }
        let reply = self.invoke(
            "vfs.preflight",
            serde_json::to_value(request).map_err(|e| e.to_string())?,
        )?;
        let decision: crate::plugin_sdk::filesystem::PreflightDecision =
            serde_json::from_value(reply)
                .map_err(|e| format!("invalid preflight response: {e}"))?;
        if decision.allowed {
            Ok(())
        } else {
            Err(format!(
                "PermissionDenied: {}",
                decision
                    .reason
                    .unwrap_or_else(|| "transfer denied by provider".into())
            ))
        }
    }

    pub fn stat(&self, node_id: &str) -> Result<FsEntry, String> {
        let value = self.invoke("vfs.stat", json!({"nodeId": node_id}))?;
        FsEntry::parse(value)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn search_stream(
        &self,
        node_id: &str,
        query: &str,
        hidden: bool,
        ignores: &[String],
        limit: usize,
        cancelled: &dyn Fn() -> bool,
        emit: &mut dyn FnMut(FsEntry) -> Result<(), String>,
    ) -> Result<bool, String> {
        let mut cursor: Option<String> = None;
        let mut seen = std::collections::HashSet::new();
        let mut count = 0usize;
        for _ in 0..MAX_PAGES {
            if cancelled() {
                return Ok(true);
            }
            let remaining = limit.saturating_sub(count);
            if remaining == 0 {
                return Ok(true);
            }
            let page = self.invoke(
                "vfs.search",
                json!({
                    "nodeId": node_id, "query": query, "mode": "name", "hidden": hidden,
                    "ignores": ignores, "limit": remaining.min(PAGE_LIMIT), "cursor": cursor,
                }),
            )?;
            let items = page["entries"]
                .as_array()
                .ok_or("provider search is missing entries")?;
            if items.len() > remaining.min(PAGE_LIMIT) {
                return Err("provider exceeded the search limit".into());
            }
            for item in items {
                if cancelled() {
                    return Ok(true);
                }
                emit(FsEntry::parse(item.clone())?)?;
                count += 1;
            }
            let next = page["nextCursor"].as_str().map(str::to_owned);
            match next {
                None => return Ok(page["truncated"].as_bool().unwrap_or(false)),
                Some(next) if !seen.insert(next.clone()) => {
                    return Err("search pagination cursor did not advance".into())
                }
                Some(next) => cursor = Some(next),
            }
        }
        Ok(true)
    }

    pub fn mkdir(&self, parent_id: &str, name: &str) -> Result<FsEntry, String> {
        crate::plugins::mounts::segment(name)?;
        let value = self.invoke("vfs.mkdir", json!({"parentId": parent_id, "name": name}))?;
        FsEntry::parse(value)
    }

    pub fn rename(
        &self,
        node_id: &str,
        name: &str,
        parent_id: Option<&str>,
    ) -> Result<FsEntry, String> {
        crate::plugins::mounts::segment(name)?;
        let value = self.invoke(
            "vfs.rename",
            json!({"nodeId": node_id, "name": name, "parentId": parent_id}),
        )?;
        FsEntry::parse(value)
    }

    pub fn remove(&self, node_id: &str) -> Result<(), String> {
        self.invoke("vfs.remove", json!({"nodeId": node_id}))?;
        Ok(())
    }

    pub fn open_read(&self, node_id: &str) -> Result<FsStream, String> {
        let value = self.invoke("vfs.openRead", json!({"nodeId": node_id}))?;
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
            crate::plugins::mounts::segment(name)?;
        }
        let value = self.invoke(
            "vfs.beginWrite",
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
        crate::plugins::provider_registry::supports(&self.plugin_id, operation)
    }
}

/// The plugin that owns a Core mount URI. Works without the desktop feature,
/// so UI-affordance code can consult declared capabilities everywhere.
pub fn mount_plugin_id(path: &str) -> Result<String, String> {
    let (_, source) = crate::plugins::mounts::resolve(path)?;
    Ok(match source {
        crate::plugins::mounts::Source::Connection(connection) => connection.plugin_id,
        crate::plugins::mounts::Source::View(view) => view.descriptor.plugin_id,
    })
}

/// Resolve a Core mount URI (`fiex://mount/<id>/..`) to the provider that
/// owns it. Used by navigation and UI affordance code; protocol calls receive a
/// prepared [`Mount`] instead.
#[cfg(feature = "desktop")]
pub fn mount_for_path(path: &str) -> Result<Mount, String> {
    let (uri, source) = crate::plugins::mounts::resolve(path)?;
    match source {
        crate::plugins::mounts::Source::Connection(connection) => {
            crate::remote::ensure_provider_access(&connection)?;
            Ok(Mount {
                mount_id: uri.mount_id,
                plugin_id: connection.plugin_id.clone(),
                source: Source::Connection(crate::remote::provider_config(&connection)),
                scope: None,
            })
        }
        crate::plugins::mounts::Source::View(view) => Ok(Mount {
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

mod streams;
mod types;
pub use streams::{FsStream, FsWriteStream};
pub use types::{FsEntry, FsKind, FsListing, WriteRequest};
#[cfg(test)]
mod tests;
