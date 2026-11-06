//! Recent's shared VFS, after-open hook, and remove-from-Recent menu.
use crate::{RecentRecord, RecentStore};
use fiex_provider_contracts::{
    ContextMenuProvider, FsEntry, FsKind, FsListing, FsOperation, FsRead, FsRequest, HookProvider,
    HookResult, MenuExecuteResult, MenuItem, MenuPlacement, ProviderError, ProviderInput,
    ProviderInvocation, SearchCapabilities, VfsProvider,
};
use fiex_storage::native::NativeScopedStorage;
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use std::{path::Path, sync::Arc};

const ROOT: &str = "recent://";
const SCOPE: &str = "history";
const URI_SCHEMES: &[&str] = &["recent"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Existence {
    Present,
    Missing,
    Unknown,
}

#[derive(Clone, Debug, Default)]
pub struct RecentResource {
    pub locator: String,
    pub name: String,
    pub extension: String,
    pub mime: String,
    pub size: Option<u64>,
    pub modified: Option<i64>,
    pub is_directory: bool,
}

/// Core supplies only the OS/resource-boundary operations. Recent storage,
/// list/stat/read policy, the successful-open hook, and its menu stay here.
pub trait RecentModuleServices: Send + Sync {
    fn storage(
        &self,
        ctx: &dyn ProviderInvocation,
        scope: &str,
    ) -> Result<NativeScopedStorage, ProviderError>;
    fn path(&self, ctx: &dyn ProviderInvocation) -> Result<String, ProviderError>;
    fn resource_for_open(
        &self,
        ctx: &dyn ProviderInvocation,
    ) -> Result<RecentResource, ProviderError>;
    fn legacy_resource(&self, locator: &str) -> Option<RecentResource>;
    fn normalize_locator(&self, locator: &str, store: &RecentStore) -> Result<String, String>;
    fn existence(&self, locator: &str) -> Existence;
    fn open_source(
        &self,
        ctx: &dyn ProviderInvocation,
        locator: &str,
    ) -> Result<FsRead, ProviderError>;
    fn selection(
        &self,
        ctx: &dyn ProviderInvocation,
    ) -> Result<(String, Vec<String>), ProviderError>;
    fn invalidate_removed_aliases(&self, aliases: &[String]);
}

/// One module instance groups its three owned provider kinds around the same
/// host adapters and storage owner.
pub struct RecentModule {
    services: Arc<dyn RecentModuleServices>,
}

impl RecentModule {
    pub fn new(services: Arc<dyn RecentModuleServices>) -> Self {
        Self { services }
    }

    pub fn vfs(&self) -> Arc<dyn VfsProvider> {
        Arc::new(RecentVfs::new(self.services.clone()))
    }

    pub fn hook(&self) -> Arc<dyn HookProvider> {
        Arc::new(RecentHook::new(self.services.clone()))
    }

    pub fn context_menu(&self) -> Arc<dyn ContextMenuProvider> {
        Arc::new(RecentContextMenu::new(self.services.clone()))
    }
}

fn store(
    services: &dyn RecentModuleServices,
    ctx: &dyn ProviderInvocation,
) -> Result<RecentStore, ProviderError> {
    let store = RecentStore::new(services.storage(ctx, SCOPE)?);
    ensure_migrated(&store, services).map_err(ProviderError::internal)?;
    Ok(store)
}

fn ensure_migrated(store: &RecentStore, services: &dyn RecentModuleServices) -> Result<(), String> {
    store.ensure_legacy_migrated(|locator| {
        let candidate = services.legacy_resource(locator)?;
        legacy_record(candidate).ok().flatten()
    })
}

fn legacy_record(resource: RecentResource) -> Result<Option<RecentRecord>, String> {
    if resource.is_directory {
        return Ok(None);
    }
    RecentRecord::new(
        resource.locator,
        resource.name,
        resource.extension,
        resource.mime,
        resource.size,
        resource.modified,
    )
    .map(Some)
}

fn recent_record(
    resource: RecentResource,
    store: &RecentStore,
    services: &dyn RecentModuleServices,
) -> Result<Option<RecentRecord>, String> {
    if resource.is_directory || resource.locator.starts_with("applications://") {
        return Ok(None);
    }
    let locator = services.normalize_locator(&resource.locator, store)?;
    RecentRecord::new(
        locator,
        resource.name,
        resource.extension,
        resource.mime,
        resource.size,
        resource.modified,
    )
    .map(Some)
}

pub fn visible_records(
    records: impl IntoIterator<Item = RecentRecord>,
    services: &dyn RecentModuleServices,
) -> Vec<RecentRecord> {
    records
        .into_iter()
        .filter(|record| services.existence(&record.locator) != Existence::Missing)
        .collect()
}

fn entry(record: &RecentRecord) -> FsEntry {
    FsEntry {
        node_id: format!("recent://{}", record.id),
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

pub struct RecentHook {
    services: Arc<dyn RecentModuleServices>,
}

impl RecentHook {
    pub fn new(services: Arc<dyn RecentModuleServices>) -> Self {
        Self { services }
    }
}

impl HookProvider for RecentHook {
    fn invoke(&self, ctx: &dyn ProviderInvocation) -> Result<HookResult, ProviderError> {
        let request = ctx
            .input()?
            .hook_request()
            .ok_or_else(|| ProviderError::invalid_params("expected HookRequest"))?;
        if request.event != "afterOpen" {
            return Err(ProviderError::invalid_params(
                "unexpected Recent hook event",
            ));
        }
        let store = store(self.services.as_ref(), ctx)?;
        let candidate = self.services.resource_for_open(ctx)?;
        if let Some(record) = recent_record(candidate, &store, self.services.as_ref())
            .map_err(ProviderError::internal)?
        {
            store.upsert(record).map_err(ProviderError::internal)?;
        }
        Ok(HookResult { handled: false })
    }
}

pub struct RecentContextMenu {
    services: Arc<dyn RecentModuleServices>,
}

impl RecentContextMenu {
    pub fn new(services: Arc<dyn RecentModuleServices>) -> Self {
        Self { services }
    }

    fn paths(&self, ctx: &dyn ProviderInvocation) -> Result<(String, Vec<String>), ProviderError> {
        self.services.selection(ctx)
    }

    fn item(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<MenuItem>, ProviderError> {
        let (_, paths) = self.paths(ctx)?;
        if paths.is_empty() || paths.iter().any(|path| !path.starts_with(ROOT)) {
            return Ok(Vec::new());
        }
        let zh = ctx.locale_tag().starts_with("zh");
        let mut item = MenuItem::action(
            "removeFromRecent",
            if paths.len() == 1 {
                if zh {
                    "从最近使用中移除"
                } else {
                    "Remove from Recent"
                }
            } else if zh {
                "从最近使用中移除选中项"
            } else {
                "Remove selected from Recent"
            },
        );
        item.icon = Some("trash".into());
        item.danger = true;
        item.placement = Some(MenuPlacement::Tail);
        Ok(vec![item])
    }
}

impl ContextMenuProvider for RecentContextMenu {
    fn resolve(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<MenuItem>, ProviderError> {
        if ctx.method() != "contextMenu.resolve" {
            return Err(ProviderError::method_not_found(ctx.method()));
        }
        self.item(ctx)
    }

    fn execute(&self, ctx: &dyn ProviderInvocation) -> Result<MenuExecuteResult, ProviderError> {
        if ctx.method() != "contextMenu.execute" {
            return Err(ProviderError::method_not_found(ctx.method()));
        }
        let (directory, paths) = self.paths(ctx)?;
        let id = match ctx.input()? {
            ProviderInput::ContextMenuExecute(request) => request.item_id.as_deref(),
            _ => None,
        }
        .ok_or_else(|| ProviderError::invalid_params("menu execute requires an itemId"))?;
        if id != "removeFromRecent"
            || paths.is_empty()
            || paths.iter().any(|path| !path.starts_with(ROOT))
        {
            return Err(ProviderError::invalid_params(
                "remove from Recent requires an offered Recent selection",
            ));
        }
        let store = store(self.services.as_ref(), ctx)?;
        let removed = store
            .remove_aliases(&paths)
            .map_err(ProviderError::internal)?;
        self.services.invalidate_removed_aliases(&removed);
        let path = paths[0].clone();
        Ok(MenuExecuteResult::core_action(
            "removeFromRecent",
            paths,
            path,
            directory,
        ))
    }
}

pub const fn recent_uri_schemes() -> &'static [&'static str] {
    URI_SCHEMES
}
