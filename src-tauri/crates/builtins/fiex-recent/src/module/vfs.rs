//! Recent's read-only VFS and alias search implementation.
use super::{store, visible_records, RecentModuleServices, ROOT};
use crate::{recent_uri, RecentRecord, RecentStore};
use fiex_provider_contracts::{
    FsEntry, FsKind, FsListing, FsOperation, FsRead, FsRequest, ProviderError, ProviderInvocation,
    SearchCapabilities, VfsProvider,
};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use std::{path::Path, sync::Arc};

fn entry(record: &RecentRecord) -> FsEntry {
    FsEntry {
        node_id: recent_uri(record),
        name: record.name.clone(),
        kind: FsKind::File,
        size: record.size,
        modified: record.modified,
        extension: (!record.extension.is_empty()).then(|| record.extension.clone()),
        mime: (!record.mime.is_empty()).then(|| record.mime.clone()),
        allowed_ops: Some(vec![FsOperation::Stat, FsOperation::Read]),
        ..Default::default()
    }
}

fn root_entry() -> FsEntry {
    FsEntry::directory(ROOT, "core:recent").allowing([
        FsOperation::List,
        FsOperation::Stat,
        FsOperation::Search,
    ])
}

fn operations(path: &str, store: &RecentStore) -> Vec<FsOperation> {
    if path == ROOT {
        return vec![FsOperation::List, FsOperation::Stat, FsOperation::Search];
    }
    store
        .record_for_alias(path)
        .map(|_| vec![FsOperation::Stat, FsOperation::Read])
        .unwrap_or_default()
}

fn provider_error(error: impl ToString) -> ProviderError {
    let message = error.to_string();
    if message.starts_with("Unsupported:") {
        ProviderError::method_not_found("vfs.operation")
    } else {
        ProviderError::invalid_params(message)
    }
}

pub struct RecentVfs {
    services: Arc<dyn RecentModuleServices>,
}

impl RecentVfs {
    pub fn new(services: Arc<dyn RecentModuleServices>) -> Self {
        Self { services }
    }
}

impl VfsProvider for RecentVfs {
    fn operations(&self) -> &'static [FsOperation] {
        &[
            FsOperation::List,
            FsOperation::Stat,
            FsOperation::Read,
            FsOperation::Search,
        ]
    }

    fn discover(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<FsOperation>, ProviderError> {
        let path = self.services.path(ctx)?;
        Ok(operations(&path, &store(self.services.as_ref(), ctx)?))
    }

    fn search_capabilities(&self, ctx: &dyn ProviderInvocation) -> SearchCapabilities {
        let Ok(path) = self.services.path(ctx) else {
            return SearchCapabilities::default();
        };
        let Ok(store) = store(self.services.as_ref(), ctx) else {
            return SearchCapabilities::default();
        };
        SearchCapabilities {
            name: operations(&path, &store).contains(&FsOperation::Search),
            content: false,
        }
    }

    fn stat(&self, request: &FsRequest<'_>) -> Result<FsEntry, ProviderError> {
        let path = self.services.path(request.ctx())?;
        if path == ROOT {
            return Ok(root_entry());
        }
        store(self.services.as_ref(), request.ctx())?
            .record_for_alias(&path)
            .map(|record| entry(&record))
            .map_err(provider_error)
    }

    fn list(&self, request: &FsRequest<'_>) -> Result<FsListing, ProviderError> {
        let path = self.services.path(request.ctx())?;
        if path != ROOT {
            return Err(ProviderError::method_not_found("vfs.list"));
        }
        let records = store(self.services.as_ref(), request.ctx())?
            .records()
            .map_err(provider_error)?;
        Ok(FsListing::new(
            visible_records(records, self.services.as_ref())
                .iter()
                .map(entry),
        ))
    }

    fn read(&self, request: &FsRequest<'_>) -> Result<FsRead, ProviderError> {
        let alias = request
            .node_id()
            .map(str::to_owned)
            .unwrap_or(self.services.path(request.ctx())?);
        let source = store(self.services.as_ref(), request.ctx())?
            .source_for(&alias)
            .map_err(provider_error)?;
        self.services.open_source(request.ctx(), &source)
    }

    fn search(&self, request: &FsRequest<'_>) -> Result<FsListing, ProviderError> {
        let path = self.services.path(request.ctx())?;
        if path != ROOT {
            return Err(ProviderError::method_not_found("vfs.search"));
        }
        let query = request
            .query()
            .filter(|query| !query.is_empty())
            .ok_or_else(|| ProviderError::invalid_params("search query is required"))?
            .to_lowercase();
        let ignores = IgnoreSet::new(&request.ignores());
        let records = store(self.services.as_ref(), request.ctx())?
            .records()
            .map_err(provider_error)?;
        let mut entries = Vec::new();
        let mut truncated = false;
        for record in visible_records(records, self.services.as_ref()) {
            if (!request.hidden() && record.name.starts_with('.'))
                || ignores.is_ignored(Path::new(&record.name))
                || !record.name.to_lowercase().contains(&query)
            {
                continue;
            }
            if entries.len() == request.limit() {
                truncated = true;
                break;
            }
            let mut result = entry(&record);
            result.relative_path = Some(record.name);
            entries.push(result);
        }
        Ok(FsListing {
            entries,
            truncated,
            ..Default::default()
        })
    }
}

struct IgnoreSet {
    names: GlobSet,
    paths: GlobSet,
}

impl IgnoreSet {
    fn new(patterns: &[String]) -> Self {
        let mut names = GlobSetBuilder::new();
        let mut paths = GlobSetBuilder::new();
        for raw in patterns {
            let pattern = raw.trim();
            if pattern.is_empty() || pattern.starts_with('#') {
                continue;
            }
            let Ok(glob) = GlobBuilder::new(pattern)
                .case_insensitive(true)
                .literal_separator(false)
                .build()
            else {
                continue;
            };
            if pattern.contains('/') {
                paths.add(glob);
            } else {
                names.add(glob);
            }
        }
        Self {
            names: names
                .build()
                .unwrap_or_else(|_| GlobSetBuilder::new().build().unwrap()),
            paths: paths
                .build()
                .unwrap_or_else(|_| GlobSetBuilder::new().build().unwrap()),
        }
    }

    fn is_ignored(&self, relative: &Path) -> bool {
        let name = relative
            .file_name()
            .map(|name| name.to_string_lossy())
            .unwrap_or_default();
        self.names.is_match(name.as_ref())
            || self
                .paths
                .is_match(relative.to_string_lossy().replace('\\', "/"))
    }
}
