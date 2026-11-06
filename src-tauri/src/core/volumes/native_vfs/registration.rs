//! Native VFS adapter and central module registration.
use super::application_provider::ApplicationView;
use super::provider_handlers::*;
use super::{recent_provider, storage_provider::StorageProvider, trash_provider};
use crate::{
    core::{files::Entry, fs::provider_registry::ChildKind, fs::ResourceBroker},
    plugins::provider_registry::native::NativeProviderRegistry,
};
use fiex_provider_contracts::{
    FsEntry, FsListing, FsOperation, FsRead, FsRequest, FsWrite, PreflightHandlers, ProviderError,
    ProviderInvocation, ProviderKind, ProviderMatchRule, ResourceType, SearchCapabilities,
    VfsProvider,
};
use fiex_runtime::ModuleDescriptor;
use fiex_storage::CoreOwnerKey;
use std::{path::PathBuf, sync::Arc};

const NO_URI_SCHEMES: &[&str] = &[];
const APPLICATIONS_URI_SCHEMES: &[&str] = &["applications"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NativeVfsIdentity {
    pub(crate) provider_id: &'static str,
    pub(crate) storage_type: Option<ResourceType>,
    pub(crate) mount_id: String,
    pub(crate) available: bool,
    pub(crate) host_path: bool,
}

/// Optional operations are implemented by the registered VFS module itself.
/// This keeps behavior attached to the provider instance instead of maintaining
/// a second table of provider IDs and function pointers.
pub(crate) trait NativeVfsBehavior: Send + Sync {
    #[cfg(feature = "desktop")]
    fn open(&self, _path: &str) -> Option<Result<(), String>> {
        None
    }
    fn validate_child(&self, _parent: &str, _child: &str) -> Option<Result<(), String>> {
        None
    }
    fn child_path(
        &self,
        _parent: &str,
        _name: &str,
        _kind: ChildKind,
    ) -> Option<Result<String, String>> {
        None
    }
    fn parent_path(&self, _path: &str) -> Option<Result<String, String>> {
        None
    }
    fn same_mount_move(
        &self,
        _sources: &[ResourceBroker],
        _destination: &ResourceBroker,
    ) -> Option<Result<Vec<String>, String>> {
        None
    }
    fn backing_path(&self, _path: &str) -> Option<PathBuf> {
        None
    }
    fn validate_binding(
        &self,
        _path: &str,
        _reference: &crate::plugins::contracts::ResourceRef,
    ) -> Option<Result<(), String>> {
        None
    }
    fn after_rename(&self, _path: &str) {}
    fn is_tree_path(&self, _path: &str) -> bool {
        false
    }
    fn resource_kind(&self, _path: &str) -> Option<&'static str> {
        None
    }
    fn icon(&self, _path: &str) -> Option<String> {
        None
    }
    fn default_view(&self, _path: &str) -> Option<&'static str> {
        None
    }
    fn open_policy(&self, _path: &str, _entry: &Entry) -> Option<bool> {
        None
    }
    #[cfg(feature = "desktop")]
    fn navigation_source(&self, _path: &str) -> Option<String> {
        None
    }
    fn provider_identity(
        &self,
        _path: &str,
        _provider_id: &'static str,
    ) -> Option<NativeVfsIdentity> {
        None
    }
    fn validate_scoped_child(
        &self,
        _scope: &crate::plugins::resource::fs_access::Scope,
        _parent: &str,
        _child: &str,
    ) -> Option<Result<(), String>> {
        None
    }
    fn revalidate_cached_listing(&self, _entries: &mut Vec<Entry>) -> Option<Result<(), String>> {
        None
    }
}

pub(crate) trait RegisteredNativeVfsProvider: VfsProvider {
    fn behavior(&self) -> &dyn NativeVfsBehavior;
}

struct NativeVfsAdapter<B> {
    provider: Arc<dyn VfsProvider>,
    behavior: B,
}

impl<B: NativeVfsBehavior> RegisteredNativeVfsProvider for NativeVfsAdapter<B> {
    fn behavior(&self) -> &dyn NativeVfsBehavior {
        &self.behavior
    }
}

impl<B: NativeVfsBehavior> VfsProvider for NativeVfsAdapter<B> {
    fn preflight_handlers(&self) -> Result<PreflightHandlers, ProviderError> {
        self.provider.preflight_handlers()
    }

    fn operations(&self) -> &'static [FsOperation] {
        self.provider.operations()
    }

    fn search_capabilities(&self, ctx: &dyn ProviderInvocation) -> SearchCapabilities {
        self.provider.search_capabilities(ctx)
    }

    fn discover(&self, ctx: &dyn ProviderInvocation) -> Result<Vec<FsOperation>, ProviderError> {
        self.provider.discover(ctx)
    }

    fn authorize(
        &self,
        ctx: &dyn ProviderInvocation,
        operation: FsOperation,
        create: bool,
    ) -> Result<(), ProviderError> {
        self.provider.authorize(ctx, operation, create)
    }

    fn list(&self, request: &FsRequest<'_>) -> Result<FsListing, ProviderError> {
        self.provider.list(request)
    }

    fn stat(&self, request: &FsRequest<'_>) -> Result<FsEntry, ProviderError> {
        self.provider.stat(request)
    }

    fn read(&self, request: &FsRequest<'_>) -> Result<FsRead, ProviderError> {
        self.provider.read(request)
    }

    fn write(&self, request: &FsRequest<'_>) -> Result<FsWrite, ProviderError> {
        self.provider.write(request)
    }

    fn mkdir(&self, request: &FsRequest<'_>) -> Result<FsEntry, ProviderError> {
        self.provider.mkdir(request)
    }

    fn rename(&self, request: &FsRequest<'_>) -> Result<FsEntry, ProviderError> {
        self.provider.rename(request)
    }

    fn remove(&self, request: &FsRequest<'_>) -> Result<(), ProviderError> {
        self.provider.remove(request)
    }

    fn trash(&self, request: &FsRequest<'_>) -> Result<(), ProviderError> {
        self.provider.trash(request)
    }

    fn search(&self, request: &FsRequest<'_>) -> Result<FsListing, ProviderError> {
        self.provider.search(request)
    }

    fn search_cancellable(
        &self,
        request: &FsRequest<'_>,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<FsListing, ProviderError> {
        self.provider.search_cancellable(request, cancelled)
    }
}

pub(crate) fn register(registry: &mut NativeProviderRegistry<dyn RegisteredNativeVfsProvider>) {
    register_provider(
        registry,
        fiex_recent::PROVIDER_ID,
        CoreOwnerKey::new("recent"),
        fiex_recent::RecentModule::new(recent_provider::services()).vfs(),
        fiex_recent::recent_uri_schemes(),
        RecentOperations,
    );
    register_provider(
        registry,
        fiex_applications::PROVIDER_ID,
        fiex_applications::owner_key(),
        Arc::new(ApplicationView),
        APPLICATIONS_URI_SCHEMES,
        ApplicationsOperations,
    );
    register_provider(
        registry,
        fiex_trash::PROVIDER_ID,
        fiex_trash::owner_key(),
        fiex_trash::TrashModule::new(trash_provider::services()).vfs(),
        fiex_trash::uri_schemes(),
        TrashOperations,
    );
    register_provider(
        registry,
        fiex_local_fs::PROVIDER_ID,
        CoreOwnerKey::new("core"),
        Arc::new(StorageProvider::new(fiex_local_fs::PROVIDER_ID)),
        NO_URI_SCHEMES,
        LocalStorageOperations,
    );
    register_provider(
        registry,
        fiex_usb_fs::PROVIDER_ID,
        CoreOwnerKey::new("core"),
        Arc::new(StorageProvider::new(fiex_usb_fs::PROVIDER_ID)),
        NO_URI_SCHEMES,
        UsbStorageOperations,
    );
}

fn register_provider<B: NativeVfsBehavior + 'static>(
    registry: &mut NativeProviderRegistry<dyn RegisteredNativeVfsProvider>,
    id: &'static str,
    owner: CoreOwnerKey,
    provider: Arc<dyn VfsProvider>,
    uri_schemes: &'static [&'static str],
    behavior: B,
) {
    let provider: Arc<dyn RegisteredNativeVfsProvider> =
        Arc::new(NativeVfsAdapter { provider, behavior });
    let mut module = registry.module(ModuleDescriptor::new(id, owner, env!("CARGO_PKG_VERSION")));
    module
        .register(id, 100, ProviderMatchRule::default(), provider)
        .expect("invalid native VFS registration");
    for scheme in uri_schemes {
        module
            .register_vfs_uri(id, scheme)
            .expect("invalid native VFS URI route");
    }
}

pub(crate) fn registry() -> NativeProviderRegistry<dyn RegisteredNativeVfsProvider> {
    crate::plugins::providers::initialize_native_providers();
    NativeProviderRegistry::shared(ProviderKind::Vfs)
}
