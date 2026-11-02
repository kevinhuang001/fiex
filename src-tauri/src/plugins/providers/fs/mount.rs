use super::{
    endpoint, proxy, types, FsEntry, FsListing, FsStream, FsWriteStream, Mount, Source,
    WriteRequest, MAX_ENTRIES, MAX_PAGES, PAGE_LIMIT,
};
use crate::plugins::provider_registry::FsOperation;
use serde_json::{json, Value};

impl Mount {
    fn invoke(&self, method: &str, params: Value) -> Result<Value, String> {
        proxy::invoke(self, method, params)
    }

    pub(super) fn invoke_remote(&self, method: &str, mut params: Value) -> Result<Value, String> {
        if !endpoint::method_allowed(self, method) {
            return Err("unsupported or unregistered VFS operation for this mount".into());
        }
        let object = params
            .as_object_mut()
            .ok_or("file-system parameters must be an object")?;
        object.insert("mountId".into(), json!(self.mount_id));
        match &self.source {
            Source::Connection(config) => {
                object.remove("contributionId");
                object.remove("viewContext");
                object.insert("connection".into(), config.clone());
            }
            Source::View {
                contribution_id,
                view_context,
            } => {
                object.remove("connection");
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

    pub(super) fn operations(&self) -> Option<Vec<FsOperation>> {
        crate::plugins::provider_registry::operations_mount(&self.plugin_id, &self.mount_id)
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
        let mut truncated = false;
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
            // The same bounded pagination DTO is accepted by Core and
            // emitted by SDK VFS implementations.
            let page: fiex_provider_contracts::VfsPage = serde_json::from_value(page)
                .map_err(|error| format!("invalid VFS listing page: {error}"))?;
            page.validate_page(PAGE_LIMIT)?;
            truncated |= page.truncated;
            for item in page.entries {
                entries.push(types::parse_entry(item)?);
                if entries.len() > MAX_ENTRIES {
                    return Err("provider returned too many entries".into());
                }
            }
            warnings.extend(page.warnings);
            let next = page.next_cursor;
            match next {
                None => {
                    return Ok(FsListing {
                        entries,
                        warnings,
                        truncated,
                        ..Default::default()
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
        if !crate::plugins::provider_registry::preflight_mount(&self.plugin_id, &self.mount_id) {
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
        types::parse_entry(value)
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
            let page: fiex_provider_contracts::VfsPage = serde_json::from_value(page)
                .map_err(|error| format!("invalid VFS search page: {error}"))?;
            page.validate_page(remaining.min(PAGE_LIMIT))?;
            for item in page.entries {
                if cancelled() {
                    return Ok(true);
                }
                emit(types::parse_entry(item)?)?;
                count += 1;
            }
            let truncated = page.truncated;
            let next = page.next_cursor;
            match next {
                None => return Ok(truncated),
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
        types::parse_entry(value)
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
        types::parse_entry(value)
    }

    pub fn remove(&self, node_id: &str) -> Result<(), String> {
        self.invoke("vfs.remove", json!({"nodeId": node_id}))?;
        Ok(())
    }

    pub fn open_read(&self, node_id: &str) -> Result<FsStream, String> {
        proxy::open_read(self, json!({"nodeId": node_id})).map(FsStream::new)
    }

    pub fn begin_write(&self, request: WriteRequest<'_>) -> Result<FsWriteStream, String> {
        if let Some(name) = request.name {
            crate::plugins::mounts::segment(name)?;
        }
        let write = proxy::begin_write(
            self,
            json!({
                "nodeId": request.node_id,
                "parentId": request.parent_id,
                "name": request.name,
                "size": request.size,
                "ifRevision": request.if_revision,
            }),
        )?;
        Ok(FsWriteStream::new(write))
    }

    /// Whether the provider declared an operation, for UI decisions.
    pub fn supports(&self, operation: FsOperation) -> bool {
        crate::plugins::provider_registry::supports_mount(
            &self.plugin_id,
            &self.mount_id,
            operation,
        )
    }
}
