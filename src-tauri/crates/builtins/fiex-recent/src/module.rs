//! Recent's shared VFS, after-open hook, and remove-from-Recent menu.
use crate::{RecentRecord, RecentStore};

mod actions;
mod vfs;
pub use actions::{RecentContextMenu, RecentHook};
use fiex_provider_contracts::{
    ContextMenuProvider, FsRead, HookProvider, ProviderError, ProviderInvocation, VfsProvider,
};
use fiex_storage::native::NativeScopedStorage;
use std::sync::Arc;
pub use vfs::RecentVfs;

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

pub(super) fn store(
    services: &dyn RecentModuleServices,
    ctx: &dyn ProviderInvocation,
) -> Result<RecentStore, ProviderError> {
    Ok(RecentStore::new(services.storage(ctx, SCOPE)?))
}

pub(super) fn recent_record(
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

pub const fn recent_uri_schemes() -> &'static [&'static str] {
    URI_SCHEMES
}
