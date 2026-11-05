//! Native VFS adapter and central module registration.
use super::application_provider::ApplicationView;
use super::provider_handlers::*;
use super::{
    application_provider, applications, recent_provider, storage_provider::StorageProvider,
    trash_provider, trash_vfs,
};
use crate::{core::files::Entry, plugins::provider_registry::native::NativeProviderRegistry};
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

#[cfg(feature = "desktop")]
pub(super) type OpenHandler = fn(&str) -> Result<(), String>;
pub(super) type ValidateChildHandler = fn(&str, &str) -> Result<(), String>;
pub(super) type ChildPathHandler =
    fn(&str, &str, crate::core::fs::provider_registry::ChildKind) -> Result<String, String>;
pub(super) type ParentPathHandler = fn(&str) -> Result<String, String>;
pub(super) type SameMountMoveHandler = fn(
    &[crate::core::fs::ResourceBroker],
    &crate::core::fs::ResourceBroker,
) -> Result<Vec<String>, String>;
pub(super) type BackingPathHandler = fn(&str) -> Option<PathBuf>;
pub(super) type BindingValidator =
    fn(&str, &crate::plugins::contracts::ResourceRef) -> Result<(), String>;
pub(super) type RenameHandler = fn(&str);
pub(super) type TreePathMatcher = fn(&str) -> bool;
pub(super) type ResourceKindResolver = fn(&str) -> &'static str;
pub(super) type IconHandler = fn(&str) -> Option<String>;
pub(super) type DefaultViewHandler = fn(&str) -> Option<&'static str>;
pub(super) type OpenPolicyHandler = fn(&str, &Entry) -> Option<bool>;
#[cfg(feature = "desktop")]
pub(super) type NavigationSourceHandler = fn(&str) -> Option<String>;
pub(super) type ProviderIdentityHandler = fn(&str, &'static str) -> Option<NativeVfsIdentity>;
pub(super) type ScopedValidateChildHandler =
    fn(&crate::plugins::resource::fs_access::Scope, &str, &str) -> Result<(), String>;
pub(super) type CachedListingHandler = fn(&mut Vec<Entry>) -> Result<(), String>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NativeVfsIdentity {
    pub(crate) provider_id: &'static str,
    pub(crate) storage_type: Option<ResourceType>,
    pub(crate) mount_id: String,
    pub(crate) available: bool,
    pub(crate) host_path: bool,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct NativeVfsExtensions {
    #[cfg(feature = "desktop")]
    pub(super) open: Option<OpenHandler>,
    pub(super) validate_child: Option<ValidateChildHandler>,
    pub(super) child_path: Option<ChildPathHandler>,
    pub(super) parent_path: Option<ParentPathHandler>,
    pub(super) same_mount_move: Option<SameMountMoveHandler>,
    pub(super) backing_path: Option<BackingPathHandler>,
    pub(super) validate_binding: Option<BindingValidator>,
    pub(super) after_rename: Option<RenameHandler>,
    pub(super) is_tree_path: Option<TreePathMatcher>,
    pub(super) resource_kind: Option<ResourceKindResolver>,
    pub(super) icon: Option<IconHandler>,
    pub(super) default_view: Option<DefaultViewHandler>,
    pub(super) open_policy: Option<OpenPolicyHandler>,
    #[cfg(feature = "desktop")]
    pub(super) navigation_source: Option<NavigationSourceHandler>,
    pub(super) provider_identity: Option<ProviderIdentityHandler>,
    pub(super) validate_scoped_child: Option<ScopedValidateChildHandler>,
    pub(super) revalidate_cached_listing: Option<CachedListingHandler>,
}

pub(crate) trait RegisteredNativeVfsProvider: VfsProvider {
    fn extensions(&self) -> &NativeVfsExtensions;
}

struct NativeVfsAdapter {
    provider: Arc<dyn VfsProvider>,
    extensions: NativeVfsExtensions,
}

impl RegisteredNativeVfsProvider for NativeVfsAdapter {
    fn extensions(&self) -> &NativeVfsExtensions {
        &self.extensions
    }
}

impl VfsProvider for NativeVfsAdapter {
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
    for (id, owner, provider, uri_schemes, provider_extensions) in [
        (
            fiex_recent::PROVIDER_ID,
            CoreOwnerKey::new("recent"),
            fiex_recent::RecentModule::new(recent_provider::services()).vfs(),
            fiex_recent::recent_uri_schemes(),
            NativeVfsExtensions {
                validate_child: Some(validate_recent_child),
                is_tree_path: Some(recent_is_tree_path),
                resource_kind: Some(recent_resource_kind),
                provider_identity: Some(recent_identity),
                #[cfg(feature = "desktop")]
                navigation_source: Some(resolve_recent_navigation_source),
                revalidate_cached_listing: Some(revalidate_recent_listing),
                ..NativeVfsExtensions::default()
            },
        ),
        (
            fiex_applications::PROVIDER_ID,
            fiex_applications::owner_key(),
            Arc::new(ApplicationView) as Arc<dyn VfsProvider>,
            APPLICATIONS_URI_SCHEMES,
            NativeVfsExtensions {
                #[cfg(feature = "desktop")]
                open: Some(applications::open),
                validate_child: Some(applications::validate_child),
                child_path: Some(applications_child_path),
                parent_path: Some(applications::parent_uri),
                same_mount_move: Some(application_provider::move_within_mount),
                backing_path: Some(application_provider::backing_path),
                after_rename: Some(applications::cancel_reservation),
                is_tree_path: Some(applications_is_tree_path),
                resource_kind: Some(applications_resource_kind),
                icon: Some(applications_icon),
                default_view: Some(applications_default_view),
                provider_identity: Some(vfs_identity),
                ..NativeVfsExtensions::default()
            },
        ),
        (
            fiex_trash::PROVIDER_ID,
            fiex_trash::owner_key(),
            fiex_trash::TrashModule::new(trash_provider::services()).vfs(),
            fiex_trash::uri_schemes(),
            NativeVfsExtensions {
                validate_child: Some(trash_vfs::validate_child),
                is_tree_path: Some(trash_vfs::is_tree),
                resource_kind: Some(trash_resource_kind),
                provider_identity: Some(vfs_identity),
                #[cfg(feature = "desktop")]
                open: Some(trash_open),
                open_policy: Some(trash_open_policy),
                ..NativeVfsExtensions::default()
            },
        ),
        (
            fiex_local_fs::PROVIDER_ID,
            CoreOwnerKey::new("core"),
            Arc::new(StorageProvider::new(fiex_local_fs::PROVIDER_ID)) as Arc<dyn VfsProvider>,
            NO_URI_SCHEMES,
            NativeVfsExtensions {
                child_path: Some(host_child_path),
                backing_path: Some(host_backing_path),
                provider_identity: Some(host_identity),
                resource_kind: Some(host_resource_kind),
                validate_scoped_child: Some(validate_host_child),
                ..NativeVfsExtensions::default()
            },
        ),
        (
            fiex_usb_fs::PROVIDER_ID,
            CoreOwnerKey::new("core"),
            Arc::new(StorageProvider::new(fiex_usb_fs::PROVIDER_ID)) as Arc<dyn VfsProvider>,
            NO_URI_SCHEMES,
            NativeVfsExtensions {
                validate_child: Some(usb_validate_child),
                child_path: Some(usb_child_path),
                validate_binding: Some(usb_ensure_current_binding),
                provider_identity: Some(usb_identity),
                resource_kind: Some(host_resource_kind),
                ..NativeVfsExtensions::default()
            },
        ),
    ] {
        let provider: Arc<dyn RegisteredNativeVfsProvider> = Arc::new(NativeVfsAdapter {
            provider,
            extensions: provider_extensions,
        });
        let mut module =
            registry.module(ModuleDescriptor::new(id, owner, env!("CARGO_PKG_VERSION")));
        module
            .register(id, 100, ProviderMatchRule::default(), provider)
            .expect("invalid native VFS registration");
        for scheme in uri_schemes {
            module
                .register_vfs_uri(id, scheme)
                .expect("invalid native VFS URI route");
        }
    }
}

pub(crate) fn registry() -> NativeProviderRegistry<dyn RegisteredNativeVfsProvider> {
    crate::plugins::providers::initialize_native_providers();
    NativeProviderRegistry::shared(ProviderKind::Vfs)
}
