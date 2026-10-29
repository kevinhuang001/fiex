//! Adapter for both standalone and connection-backed plugin storage. Writes
//! always commit through the owning provider; no materialized cache is writable.
use super::*;
use crate::plugins::resolvers::fs::{
    FsEntry, FsKind, FsStream, FsWriteStream, Mount, WriteRequest,
};
use serde_json::json;
mod writing;
use writing::{DeferredWrite, PluginWrite};

pub(super) struct PluginFsAdapter<'a> {
    resource: &'a ResourceBroker,
}
impl<'a> PluginFsAdapter<'a> {
    pub fn new(resource: &'a ResourceBroker) -> Result<Self, String> {
        crate::plugins::mounts::resolve(&resource.path)?;
        Ok(Self { resource })
    }
    fn node(&self) -> Result<(Mount, String), String> {
        if crate::remote::is_remote(&self.resource.path) {
            crate::remote::adapter_node(&self.resource.path)
        } else {
            crate::plugins::vfs::adapter_node(&self.resource.path)
        }
    }
    fn parent(&self) -> Result<(Mount, String, String), String> {
        let uri = crate::plugins::mounts::parse(&self.resource.path)?;
        let (relative, name) = uri.relative.rsplit_once('/').unwrap_or(("", &uri.relative));
        crate::plugins::mounts::segment(name)?;
        let path = crate::plugins::mounts::uri(&uri.mount_id, relative)?;
        let reference = self.resource.scope.reference_for_path(&path)?;
        let parent =
            ResourceBroker::resolve(&self.resource.scope, &self.resource.owner, &reference)?;
        let adapter = PluginFsAdapter { resource: &parent };
        let (mount, node) = adapter.node()?;
        Ok((mount, node, name.into()))
    }
    fn remember(&self, entry: &FsEntry) -> Result<(), String> {
        if crate::remote::is_remote(&self.resource.path) {
            crate::remote::remember_adapter_entry(&self.resource.path, entry)
        } else {
            let uri = crate::plugins::mounts::parse(&self.resource.path)?;
            crate::plugins::mounts::bind_node(&uri.mount_id, &uri.relative, &entry.node_id)
        }
    }
    fn project(&self, entry: FsEntry, path: &str) -> Value {
        json!({"path":path,"name":entry.name,"isDir":entry.kind == FsKind::Directory,
            "symlink":entry.kind == FsKind::Symlink,"size":entry.size,"modified":entry.modified,
            "extension":entry.extension,"mime":entry.mime,"revision":entry.revision})
    }
}
impl FsAdapter for PluginFsAdapter<'_> {
    fn desktop_entry(&self) -> Result<crate::core::files::Entry, FsError> {
        if crate::remote::is_remote(&self.resource.path) {
            crate::remote::stat(&self.resource.path).map_err(FsError::from)
        } else {
            crate::plugins::vfs::stat(&self.resource.path).map_err(FsError::from)
        }
    }
    fn desktop_listing(
        &self,
        path: &str,
        hidden: bool,
        ignores: &[String],
    ) -> Result<crate::core::files::Listing, FsError> {
        if crate::remote::is_remote(path) {
            futures::executor::block_on(crate::remote::list_listing(path, hidden, ignores))?
                .ok_or_else(|| "MountNotFound: connection removed".into())
        } else {
            crate::plugins::vfs::list(path, hidden, ignores).map_err(FsError::from)
        }
    }
    fn stat(&self) -> Result<Value, FsError> {
        let (mount, node) = self.node()?;
        Ok(self.project(mount.stat(&node)?, &self.resource.path))
    }
    fn list(&self) -> Result<Value, FsError> {
        let (mount, node) = self.node()?;
        let uri = crate::plugins::mounts::parse(&self.resource.path)?;
        let mut out = Vec::new();
        for entry in mount.list(&node, true, &[])?.entries {
            crate::plugins::mounts::segment(&entry.name)?;
            let relative = format!("{}/{}", uri.relative.trim_end_matches('/'), entry.name);
            let path =
                crate::plugins::mounts::uri(&uri.mount_id, relative.trim_start_matches('/'))?;
            if !crate::remote::is_remote(&self.resource.path) {
                crate::plugins::mounts::bind_node(
                    &uri.mount_id,
                    relative.trim_start_matches('/'),
                    &entry.node_id,
                )?;
            }
            out.push(self.project(entry, &path));
        }
        Ok(json!(out))
    }
    fn search(
        &self,
        query: &str,
        hidden: bool,
        ignores: &[String],
        limit: usize,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(Vec<crate::core::files::Entry>, bool, usize), FsError> {
        let (mount, node) = self.node()?;
        if !crate::remote::is_remote(&self.resource.path)
            && mount.supports_search_mode(&node, "name")? {
            let (entries, truncated) =
                crate::plugins::vfs::search(&self.resource.path, query, hidden, ignores, limit)?;
            return Ok((entries, truncated, 0));
        }
        let mut pending = vec![self.resource.path.clone()];
        let mut entries = Vec::new();
        let mut visited = 0usize;
        let mut seen = std::collections::HashSet::new();
        while let Some(path) = pending.pop() {
            if cancelled() || visited >= 1000 || entries.len() >= limit {
                break;
            }
            if !seen.insert(path.clone()) { continue; }
            visited += 1;
            let listing = if crate::remote::is_remote(&path) {
                futures::executor::block_on(crate::remote::list_listing(&path, hidden, ignores))?
                    .ok_or("MountNotFound: connection removed")?
            } else {
                crate::plugins::vfs::list(&path, hidden, ignores)?
            };
            for entry in listing.entries {
                if cancelled() || entries.len() >= limit {
                    break;
                }
                if entry.is_dir {
                    pending.push(entry.path.clone());
                }
                if entry.name.to_lowercase().contains(&query.to_lowercase()) {
                    entries.push(entry);
                }
            }
        }
        let truncated = !pending.is_empty() || visited >= 1000 || entries.len() >= limit;
        Ok((entries, truncated, 0))
    }
    fn require(&self, op: FsOperation, create: bool) -> Result<(), FsError> {
        let (mount, node) = if create {
            let (mount, parent, _) = self.parent()?;
            (mount, parent)
        } else {
            self.node()?
        };
        let op = if op == FsOperation::Search && !mount.supports_search_mode(&node, "name")? {
            FsOperation::List
        } else { op };
        if !mount.supports(op) {
            return Err(FsError::new(
                FsErrorKind::Unsupported,
                format!("Unsupported: {}", op.as_str()),
            ));
        }
        let entry = mount.stat(&node)?;
        // Creating a child file needs the parent's child-create capability,
        // not the directory's own file-content Write capability.
        let required = if create && op == FsOperation::Write {
            FsOperation::Mkdir
        } else {
            op
        };
        if !entry.allows(required) {
            return Err(FsError::new(
                FsErrorKind::Unsupported,
                format!("Unsupported: node does not permit {}", required.as_str()),
            ));
        }
        // readonly is presentation metadata. The owning provider enforces real
        // server ACLs through Discovery and again when executing the operation.
        Ok(())
    }
    fn open_read(&self) -> Result<Box<dyn ReadHandle>, FsError> {
        let (mount, node) = self.node()?;
        let stream = mount.open_read(&node)?;
        let size = stream
            .size()
            .or(mount.stat(&node)?.size)
            .ok_or("Unsupported: stream size is unknown")?;
        Ok(Box::new(PluginRead {
            stream: Some(stream),
            mount,
            node,
            size,
            offset: 0,
        }))
    }
    fn begin_write(&self, create: bool) -> Result<Box<dyn WriteHandle>, FsError> {
        Ok(Box::new(DeferredWrite::new(self.resource, create)?))
    }
    fn begin_write_sized(
        &self,
        create: bool,
        size: Option<u64>,
    ) -> Result<Box<dyn WriteHandle>, FsError> {
        let stream = if create {
            let (mount, parent, name) = self.parent()?;
            mount.begin_write(WriteRequest {
                node_id: None,
                parent_id: Some(&parent),
                name: Some(&name),
                size,
                if_revision: None,
            })?
        } else {
            let (mount, node) = self.node()?;
            let entry = mount.stat(&node)?;
            mount.begin_write(WriteRequest {
                node_id: Some(&node),
                parent_id: None,
                name: None,
                size,
                if_revision: entry.revision.as_deref(),
            })?
        };
        Ok(Box::new(PluginWrite::new(self.resource, stream)))
    }
    fn mkdir(&self) -> Result<Value, FsError> {
        let (mount, parent, name) = self.parent()?;
        let entry = mount.mkdir(&parent, &name)?;
        self.remember(&entry)?;
        self.stat()
    }
    fn remove(&self) -> Result<(), FsError> {
        #[cfg(feature = "desktop")]
        if crate::remote::is_remote(&self.resource.path) {
            return futures::executor::block_on(crate::remote::remove(&self.resource.path))
                .map_err(FsError::from);
        }
        let (mount, node) = self.node()?;
        mount.remove(&node).map_err(FsError::from)
    }
    fn rename(&self, destination: &ResourceBroker) -> Result<(), FsError> {
        let target = PluginFsAdapter::new(destination)?;
        let (_, _, name) = target.parent()?;
        #[cfg(feature = "desktop")]
        if crate::remote::is_remote(&self.resource.path) {
            futures::executor::block_on(crate::remote::rename(&self.resource.path, &name))?;
            return Ok(());
        }
        let (mount, node) = self.node()?;
        let (_, parent, _) = target.parent()?;
        mount.rename(&node, &name, Some(&parent))?;
        Ok(())
    }
}
struct PluginRead {
    stream: Option<FsStream>,
    mount: Mount,
    node: String,
    size: u64,
    offset: u64,
}
impl ReadHandle for PluginRead {
    fn size(&self) -> u64 {
        self.size
    }
    fn read_at(&mut self, offset: u64, length: usize) -> Result<(Vec<u8>, bool), String> {
        if offset >= self.size {
            return Ok((vec![], true));
        }
        // A sequential provider can seek by reopening/replaying bounded chunks.
        if offset < self.offset {
            self.stream.take();
            self.stream = Some(self.mount.open_read(&self.node)?);
            self.offset = 0;
        }
        let stream = self.stream.as_ref().ok_or("read handle closed")?;
        while self.offset < offset {
            let want = (offset - self.offset).min(CHUNK as u64) as usize;
            let (bytes, eof) = stream.read_at(self.offset, want)?;
            self.offset += bytes.len() as u64;
            if eof || bytes.is_empty() {
                return Err("Conflict: stream ended before requested offset".into());
            }
        }
        let result = stream.read_at(offset, length.min(CHUNK))?;
        self.offset = self
            .offset
            .checked_add(result.0.len() as u64)
            .ok_or("read offset overflow")?;
        Ok(result)
    }
}
