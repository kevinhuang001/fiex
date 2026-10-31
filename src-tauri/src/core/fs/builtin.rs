//! Core-owned virtual storage adapter; never turns a view URI into a host path.
use super::*;
mod handles;
use crate::core::{
    recent,
    volumes::{applications, virtual_resolvers, virtual_storage},
};
use handles::{AppRead, AppWrite};
use serde_json::{json, to_value};
use std::{fs, path::PathBuf};

pub(super) struct BuiltinFsAdapter<'a>(pub &'a ResourceBroker);
impl BuiltinFsAdapter<'_> {
    fn path(&self) -> &str {
        &self.0.path
    }
    fn application_file(&self) -> Result<PathBuf, String> {
        if !applications::is_user(self.path()) {
            return Err("Unsupported: system application has no file stream".into());
        }
        applications::candidate_path(self.path())
    }

}
impl FsAdapter for BuiltinFsAdapter<'_> {
    fn preflight(
        &self,
        request: &crate::plugin_sdk::filesystem::TransferPreflight,
    ) -> Result<(), FsError> {
        let provider = virtual_resolvers::view_for(self.path())
            .ok_or("Unsupported: unregistered native VFS provider")?;
        provider.preflight(self.path(), request).map_err(FsError::from)
    }
    fn desktop_listing(
        &self,
        path: &str,
        hidden: bool,
        ignores: &[String],
    ) -> Result<crate::core::files::Listing, FsError> {
        let mut listing = super::presentation::listing(self, path, hidden, ignores)?;
        if let Some(resolver) = virtual_resolvers::view_for(path) {
            listing.read_only = resolver.readonly();
            listing.default_view = resolver.default_view().map(str::to_owned);
        }
        Ok(listing)
    }
    fn stat(&self) -> Result<Value, FsError> {
        if applications::is_user(self.path()) && !self.application_file()?.exists() {
            return Err("NotFound: application item absent".into());
        }
        to_value(virtual_storage::entry(self.path())?).map_err(|e| FsError::internal(e.to_string()))
    }
    fn list(&self) -> Result<Value, FsError> {
        to_value(virtual_storage::list(self.path())?).map_err(|e| FsError::internal(e.to_string()))
    }
    fn search(
        &self,
        query: &str,
        hidden: bool,
        ignores: &[String],
        limit: usize,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(Vec<crate::core::files::Entry>, bool, usize), FsError> {
        let mut entries = Vec::new();
        let (truncated, skipped) =
            self.search_stream(query, hidden, ignores, limit, cancelled, &mut |entry| {
                entries.push(entry)
            })?;
        Ok((entries, truncated, skipped))
    }
    #[allow(clippy::too_many_arguments)]
    fn search_stream(
        &self,
        query: &str,
        hidden: bool,
        ignores: &[String],
        limit: usize,
        cancelled: &dyn Fn() -> bool,
        emit: &mut dyn FnMut(crate::core::files::Entry),
    ) -> Result<(bool, usize), FsError> {
        let mut pending = vec![self.path().to_string()];
        let mut visited = std::collections::HashSet::new();
        let mut count = 0usize;
        let filter = crate::core::ignores::IgnoreSet::new(ignores);
        let mut truncated = false;
        while let Some(path) = pending.pop() {
            if cancelled() {
                break;
            }
            if !visited.insert(path.clone()) {
                continue;
            }
            if visited.len() > 1000 {
                truncated = true;
                break;
            }
            let listing =
                super::binding::trusted(&path, "core:search")?.desktop_listing(true, &[])?;
            for entry in listing.entries {
                if cancelled() {
                    break;
                }
                if (!hidden && entry.name.starts_with('.'))
                    || filter.is_ignored(std::path::Path::new(&entry.name))
                {
                    continue;
                }
                // Traverse only entries whose storage provider declares
                // list support. No per-view URI or directory-tree special case.
                if entry.is_dir
                    && !entry.is_symlink
                    && crate::plugins::resource::file_capabilities::for_entry(
                        &entry.path,
                        &entry,
                    )
                    .list
                {
                    pending.push(entry.path.clone());
                }
                if entry.name.to_lowercase().contains(&query.to_lowercase()) {
                    if count == limit {
                        truncated = true;
                        break;
                    }
                    count += 1;
                    emit(entry);
                }
            }
            if truncated {
                break;
            }
        }
        Ok((truncated, 0))
    }

    fn require(&self, op: FsOperation, create: bool) -> Result<(), FsError> {
        let provider = virtual_resolvers::view_for(self.path())
            .ok_or("Unsupported: unregistered native VFS provider")?;
        provider
            .require(self.path(), &self.0.owner, op, create)
            .map_err(FsError::from)
    }
    fn open_read(&self) -> Result<Box<dyn ReadHandle>, FsError> {
        self.require(FsOperation::Read, false)?;
        if self.path().starts_with(virtual_resolvers::RECENT_URI) {
            let source = recent::source_for(self.path())?;
            let original = super::binding::trusted(&source, &self.0.owner)?;
            original.require(FsOperation::Read, false)?;
            return original.adapter()?.open_read();
        }
        let file = fs::File::open(self.application_file()?)
            .map_err(|e| FsError::internal(e.to_string()))?;
        let size = file
            .metadata()
            .map_err(|e| FsError::internal(e.to_string()))?
            .len();
        Ok(Box::new(AppRead { file, size }))
    }
    fn begin_write(&self, create: bool) -> Result<Box<dyn WriteHandle>, FsError> {
        self.require(FsOperation::Write, create)?;
        let path = self.application_file()?;
        let temporary = tempfile::NamedTempFile::new_in(path.parent().ok_or("missing parent")?)
            .map_err(|e| FsError::internal(e.to_string()))?;
        Ok(Box::new(AppWrite {
            temporary: Some(temporary),
            target: path,
            create,
        }))
    }
    fn mkdir(&self) -> Result<Value, FsError> {
        self.require(FsOperation::Mkdir, true)?;
        fs::create_dir(self.application_file()?).map_err(|e| FsError::internal(e.to_string()))?;
        self.stat()
    }
    fn remove(&self) -> Result<(), FsError> {
        self.require(FsOperation::Remove, false)?;
        applications::remove_user(self.path()).map_err(FsError::from)
    }
    fn rename(&self, destination: &ResourceBroker) -> Result<(), FsError> {
        self.require(FsOperation::Rename, false)?;
        if !applications::is_user(&destination.path) {
            return Err("ScopeDenied: cannot rename into another view".into());
        }
        let from = self.application_file()?;
        let to = applications::candidate_path(&destination.path)?;
        if from.parent() != to.parent() {
            return Err("Unsupported: moving application items requires TransferEngine".into());
        }
        if to.exists() {
            return Err("Conflict: application destination exists".into());
        }
        let directory = from.is_dir();
        fs::rename(from, to).map_err(|e| FsError::internal(e.to_string()))?;
        applications::after_rename(self.path(), &destination.path, directory)
            .map_err(FsError::from)?;
        Ok(())
    }
}
