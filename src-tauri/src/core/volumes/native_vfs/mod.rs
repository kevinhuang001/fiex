//! Native storage providers implement the same shared VFS contracts as plugin providers.
use super::{applications, recycle, trash_vfs};
use crate::{core::files::Entry, plugins::provider_registry::native::NativeProviderRegistry};
use fiex_provider_contracts::{
    FsEntry, FsListing, FsOperation, FsRead, FsRequest, FsWrite, PreflightHandlers, ProviderError,
    ProviderInvocation, ProviderKind, ProviderMatchRule, ResourceType, SearchCapabilities,
    VfsProvider,
};
use fiex_runtime::ModuleDescriptor;
use fiex_storage::CoreOwnerKey;
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

mod application_provider;
mod broker;
mod presentation;
mod recent_provider;
mod shared;
mod storage;
mod storage_provider;
mod trash_provider;

use application_provider::ApplicationView;
pub use broker::{
    list_for, mkdir_for, preflight_for, read_for, remove_for, rename_for, require_for, search_for,
    stat_for, trash_for, write_sized_for,
};
#[cfg(test)]
pub(crate) use presentation::navigable_source;
pub(crate) use presentation::recent_navigation_source;
pub use presentation::{default_view, icon_for, opaque, resource_entry};
pub use shared::core_entry;
use shared::target_invocation;
use storage_provider::StorageProvider;

pub const RECENT_URI: &str = "recent://";
pub const APPLICATIONS_URI: &str = applications::URI;
const NO_URI_SCHEMES: &[&str] = &[];
const APPLICATIONS_URI_SCHEMES: &[&str] = &["applications"];

#[cfg(feature = "desktop")]
type OpenHandler = fn(&str) -> Result<(), String>;
type ValidateChildHandler = fn(&str, &str) -> Result<(), String>;
type ChildPathHandler =
    fn(&str, &str, crate::core::fs::provider_registry::ChildKind) -> Result<String, String>;
type ParentPathHandler = fn(&str) -> Result<String, String>;
type SameMountMoveHandler = fn(
    &[crate::core::fs::ResourceBroker],
    &crate::core::fs::ResourceBroker,
) -> Result<Vec<String>, String>;
type BackingPathHandler = fn(&str) -> Option<PathBuf>;
type BindingValidator = fn(&str, &crate::plugins::contracts::ResourceRef) -> Result<(), String>;
type RenameHandler = fn(&str);
type TreePathMatcher = fn(&str) -> bool;
type IconHandler = fn(&str) -> Option<String>;
type DefaultViewHandler = fn(&str) -> Option<&'static str>;
type OpenPolicyHandler = fn(&str, &Entry) -> Option<bool>;
#[cfg(feature = "desktop")]
type NavigationSourceHandler = fn(&str) -> Option<String>;
type ProviderIdentityHandler = fn(&str, &'static str) -> Option<NativeVfsIdentity>;
type ScopedValidateChildHandler =
    fn(&crate::plugins::resource::fs_access::Scope, &str, &str) -> Result<(), String>;
type CachedListingHandler = fn(&mut Vec<Entry>) -> Result<(), String>;

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
    open: Option<OpenHandler>,
    validate_child: Option<ValidateChildHandler>,
    child_path: Option<ChildPathHandler>,
    parent_path: Option<ParentPathHandler>,
    same_mount_move: Option<SameMountMoveHandler>,
    backing_path: Option<BackingPathHandler>,
    validate_binding: Option<BindingValidator>,
    after_rename: Option<RenameHandler>,
    is_tree_path: Option<TreePathMatcher>,
    icon: Option<IconHandler>,
    default_view: Option<DefaultViewHandler>,
    open_policy: Option<OpenPolicyHandler>,
    #[cfg(feature = "desktop")]
    navigation_source: Option<NavigationSourceHandler>,
    provider_identity: Option<ProviderIdentityHandler>,
    validate_scoped_child: Option<ScopedValidateChildHandler>,
    revalidate_cached_listing: Option<CachedListingHandler>,
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

pub(crate) fn recent_services() -> Arc<dyn fiex_recent::RecentModuleServices> {
    recent_provider::services()
}

pub(crate) fn trash_services() -> Arc<dyn fiex_trash::TrashModuleServices> {
    trash_provider::services()
}

fn registry() -> NativeProviderRegistry<dyn RegisteredNativeVfsProvider> {
    crate::plugins::providers::initialize_native_providers();
    NativeProviderRegistry::shared(ProviderKind::Vfs)
}

/// Resolve the native VFS identity at the URI boundary. General services use
/// the returned descriptor ID and shared provider instance thereafter.
pub fn provider_id(path: &str) -> Option<&'static str> {
    registry().route_uri(path)
}

/// Resolve a Core storage provider without promoting unknown URI schemes to
/// local paths. Local and USB resources use the same shared trait registry as
/// the built-in virtual views.
pub fn shared_provider_id(path: &str) -> Option<&'static str> {
    if let Some(provider) = provider_id(path) {
        return Some(provider);
    }
    if crate::usbfs::route(path).is_some() {
        return Some(fiex_usb_fs::PROVIDER_ID);
    }
    if path.is_empty()
        || path.contains("://")
        || crate::plugins::mounts::handles(path)
        || unregistered_scheme(path)
    {
        return None;
    }
    Some(fiex_local_fs::PROVIDER_ID)
}

/// Resolve storage and presentation identity from the owning native module's
/// registered descriptor. Providers without backing storage, such as Recent,
/// still have a stable mount identity for references and cache validation.
pub(crate) fn identity(path: &str) -> Option<NativeVfsIdentity> {
    let provider_id = shared_provider_id(path)?;
    let provider = registry().get(provider_id)?;
    (provider.extensions().provider_identity?)(path, provider_id)
}

pub(crate) fn storage_identity(path: &str) -> Option<NativeVfsIdentity> {
    identity(path).filter(|identity| identity.storage_type.is_some())
}

pub(crate) fn is_host_path(path: &str) -> bool {
    identity(path).is_some_and(|identity| identity.host_path)
}

pub(crate) fn is_filesystem_path(path: &str) -> bool {
    identity(path).is_some_and(|identity| identity.storage_type == Some(ResourceType::Fs))
}

pub(crate) fn reference_mount_id(path: &str) -> Option<String> {
    identity(path).map(|identity| identity.mount_id)
}

pub(crate) fn validate_child_with_scope(
    scope: &crate::plugins::resource::fs_access::Scope,
    parent: &str,
    child: &str,
) -> Result<(), String> {
    let provider_id =
        shared_provider_id(parent).ok_or("Unsupported: provider is not registered")?;
    if let Some(validate) = registry()
        .get(provider_id)
        .and_then(|provider| provider.extensions().validate_scoped_child)
    {
        return validate(scope, parent, child);
    }
    validate_child(parent, child)
}

pub(crate) fn revalidate_cached_listing(
    path: &str,
    entries: &mut Vec<Entry>,
) -> Result<(), String> {
    let Some(provider_id) = shared_provider_id(path) else {
        return Ok(());
    };
    let Some(revalidate) = registry()
        .get(provider_id)
        .and_then(|provider| provider.extensions().revalidate_cached_listing)
    else {
        return Ok(());
    };
    revalidate(entries)
}

pub(crate) fn open_policy(path: &str, entry: &Entry) -> Option<bool> {
    let provider_id = shared_provider_id(path)?;
    let provider = registry().get(provider_id)?;
    (provider.extensions().open_policy?)(path, entry)
}

#[cfg(feature = "desktop")]
pub(crate) fn navigation_source(path: &str) -> Option<String> {
    let provider_id = shared_provider_id(path)?;
    let provider = registry().get(provider_id)?;
    (provider.extensions().navigation_source?)(path)
}

fn recent_identity(_path: &str, provider_id: &'static str) -> Option<NativeVfsIdentity> {
    Some(NativeVfsIdentity {
        provider_id,
        storage_type: None,
        mount_id: provider_id.into(),
        available: true,
        host_path: false,
    })
}

fn vfs_identity(_path: &str, provider_id: &'static str) -> Option<NativeVfsIdentity> {
    Some(NativeVfsIdentity {
        provider_id,
        storage_type: Some(ResourceType::Vfs),
        mount_id: provider_id.into(),
        available: true,
        host_path: false,
    })
}

fn host_identity(_path: &str, provider_id: &'static str) -> Option<NativeVfsIdentity> {
    Some(NativeVfsIdentity {
        provider_id,
        storage_type: Some(ResourceType::Fs),
        mount_id: "local".into(),
        available: true,
        host_path: true,
    })
}

fn usb_identity(path: &str, provider_id: &'static str) -> Option<NativeVfsIdentity> {
    let route = crate::usbfs::route(path)?;
    let generation = crate::usbfs::generation(&route).unwrap_or_else(|_| "offline".into());
    Some(NativeVfsIdentity {
        provider_id,
        storage_type: Some(ResourceType::Fs),
        available: generation != "offline",
        mount_id: format!("core:usb:{generation}"),
        host_path: false,
    })
}

fn validate_host_child(
    scope: &crate::plugins::resource::fs_access::Scope,
    parent: &str,
    child: &str,
) -> Result<(), String> {
    scope.authorize(child)?;
    if !Path::new(child).starts_with(parent) {
        return Err("child outside selected resource".into());
    }
    Ok(())
}

fn revalidate_recent_listing(entries: &mut Vec<Entry>) -> Result<(), String> {
    let storage = crate::core::volumes::native_vfs::management_storage_for_provider(
        fiex_recent::PROVIDER_ID,
        "history",
    )?;
    crate::core::recent::filter_cached_entries(entries, &storage)
}

#[cfg(feature = "desktop")]
fn resolve_recent_navigation_source(alias: &str) -> Option<String> {
    crate::core::recent::source_for(alias).ok()
}

fn trash_open_policy(_path: &str, entry: &Entry) -> Option<bool> {
    Some(
        entry.is_dir
            && !entry.is_symlink
            && entry
                .allowed_ops
                .iter()
                .any(|operation| operation == "list"),
    )
}

#[cfg(feature = "desktop")]
fn trash_open(_path: &str) -> Result<(), String> {
    Err(crate::i18n::t("回收站项目请使用“放回原处”或“彻底删除”").into())
}

/// Return a Core-local backing path only when the owning native provider
/// deliberately exposes one. Plugin mounts and projected virtual identities
/// never become host paths through this resolver.
pub fn backing_path(path: &str) -> Option<PathBuf> {
    let provider_id = shared_provider_id(path)?;
    let provider = registry().get(provider_id)?;
    (provider.extensions().backing_path?)(path)
}

/// Let a registered native provider handle resources whose platform open
/// target is not a byte stream. Other resources continue through the ordinary
/// export/open path; Open is not an FsOperation capability.
#[cfg(feature = "desktop")]
pub(crate) fn open(path: &str) -> Result<bool, String> {
    let Some(provider) = shared_provider_id(path) else {
        return Ok(false);
    };
    match registry()
        .get(provider)
        .and_then(|view| view.extensions().open)
    {
        Some(open) => open(path).map(|()| true),
        None => Ok(false),
    }
}

/// Give native providers first chance to handle an intra-mount move as a
/// metadata operation. `None` keeps the normal TransferEngine stream path.
pub(crate) fn move_within_mount(
    sources: &[crate::core::fs::ResourceBroker],
    destination: &crate::core::fs::ResourceBroker,
) -> Result<Option<Vec<String>>, String> {
    let Some(provider) = shared_provider_id(&destination.path) else {
        return Ok(None);
    };
    if sources.iter().any(|source| {
        shared_provider_id(&source.path) != Some(provider)
            || source.reference.mount_id != destination.reference.mount_id
    }) {
        return Ok(None);
    }
    let Some(move_within_mount) = registry()
        .get(provider)
        .and_then(|view| view.extensions().same_mount_move)
    else {
        return Ok(None);
    };
    move_within_mount(sources, destination).map(Some)
}

/// USB references are scoped to a mount generation. Keep that liveness check
/// at the native storage boundary so generic ResourceBroker code stays free
/// of USB-specific policy.
pub(crate) fn ensure_current_binding(
    path: &str,
    reference: &crate::plugins::contracts::ResourceRef,
) -> Result<(), String> {
    let Some(provider_id) = shared_provider_id(path) else {
        return Ok(());
    };
    let Some(validate) = registry()
        .get(provider_id)
        .and_then(|provider| provider.extensions().validate_binding)
    else {
        return Ok(());
    };
    validate(path, reference)
}

fn usb_ensure_current_binding(
    path: &str,
    reference: &crate::plugins::contracts::ResourceRef,
) -> Result<(), String> {
    let current_mount = crate::core::fs::provider_registry::reference_mount_id(path)
        .ok_or("StaleRef: USB mount is no longer available")?;
    if reference.mount_id != current_mount {
        return Err("StaleRef: USB mount generation changed".into());
    }
    Ok(())
}

fn unregistered_scheme(path: &str) -> bool {
    let Some(index) = path.find(':') else {
        return false;
    };
    let prefix = &path[..index];
    if prefix.len() == 1 && prefix.as_bytes()[0].is_ascii_alphabetic() {
        return false;
    }
    !prefix.is_empty()
        && prefix.as_bytes()[0].is_ascii_alphabetic()
        && prefix
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
}

pub fn is_tree_path(path: &str) -> bool {
    let Some(provider_id) = shared_provider_id(path) else {
        return false;
    };
    let Some(matches) = registry()
        .get(provider_id)
        .and_then(|provider| provider.extensions().is_tree_path)
    else {
        return false;
    };
    matches(path)
}

fn applications_is_tree_path(path: &str) -> bool {
    path == APPLICATIONS_URI
        || applications::is_folder(path)
        || applications::is_app(path)
        || applications::is_user(path)
}

fn recent_is_tree_path(path: &str) -> bool {
    path == RECENT_URI
}

fn applications_icon(path: &str) -> Option<String> {
    if applications::is_user(path) {
        return None;
    }
    let entry = applications::system_entry(path).ok()?;
    applications::cached_icon(entry.link_target.as_deref()?)
}

fn applications_default_view(_path: &str) -> Option<&'static str> {
    Some("icons")
}

fn host_backing_path(path: &str) -> Option<PathBuf> {
    Some(PathBuf::from(path))
}

pub(crate) fn after_rename(path: &str) {
    let Some(provider_id) = shared_provider_id(path) else {
        return;
    };
    if let Some(after_rename) = registry()
        .get(provider_id)
        .and_then(|provider| provider.extensions().after_rename)
    {
        after_rename(path);
    }
}

/// Ask the owning native view to validate a child identity. Ordinary host paths
/// are validated by the enclosing operation scope in Core's VFS router.
pub fn validate_child(parent: &str, child: &str) -> Result<(), String> {
    let provider = shared_provider_id(parent).ok_or("Unsupported: provider is not registered")?;
    registry()
        .get(provider)
        .and_then(|view| view.extensions().validate_child)
        .ok_or("Unsupported: provider does not support child identities")?(parent, child)
}

pub(crate) fn child_path(
    parent: &str,
    name: &str,
    kind: crate::core::fs::provider_registry::ChildKind,
) -> Result<String, String> {
    let provider = shared_provider_id(parent).ok_or("Unsupported: provider is not registered")?;
    registry()
        .get(provider)
        .and_then(|view| view.extensions().child_path)
        .ok_or("Unsupported: provider does not support child creation")?(parent, name, kind)
}

pub fn parent_path(path: &str) -> Result<String, String> {
    if let Some(provider) = shared_provider_id(path) {
        if let Some(parent_path) = registry()
            .get(provider)
            .and_then(|view| view.extensions().parent_path)
        {
            return parent_path(path);
        }
    }
    if opaque(path) {
        return Err("view has no writable parent".into());
    }
    Path::new(path)
        .parent()
        .map(|path| path.to_string_lossy().into_owned())
        .ok_or_else(|| "item has no parent".into())
}

fn validate_recent_child(parent: &str, child: &str) -> Result<(), String> {
    if parent != RECENT_URI {
        return Err("recent view has no writable child identities".into());
    }
    crate::core::recent::source_for(child).map(|_| ())
}

fn host_child_path(
    parent: &str,
    name: &str,
    _kind: crate::core::fs::provider_registry::ChildKind,
) -> Result<String, String> {
    Ok(PathBuf::from(parent)
        .join(name)
        .to_string_lossy()
        .into_owned())
}

fn usb_validate_child(parent: &str, child: &str) -> Result<(), String> {
    crate::usbfs::validate_child(parent, child)
}

fn usb_child_path(
    parent: &str,
    name: &str,
    _kind: crate::core::fs::provider_registry::ChildKind,
) -> Result<String, String> {
    crate::usbfs::route(parent).ok_or("invalid USB parent")?;
    Ok(format!("{}/{}", parent.trim_end_matches(['/', '\\']), name))
}

fn applications_child_path(
    parent: &str,
    name: &str,
    kind: crate::core::fs::provider_registry::ChildKind,
) -> Result<String, String> {
    let is_directory = matches!(kind, crate::core::fs::provider_registry::ChildKind::Folder);
    applications::reserve_child(parent, name, is_directory)
}

pub fn tree_path_key(path: &str) -> Option<String> {
    is_tree_path(path).then(|| path.to_owned())
}

pub(crate) fn view_for(path: &str) -> Option<Arc<dyn RegisteredNativeVfsProvider>> {
    registry().get(provider_id(path)?)
}

fn shared_view_for(path: &str) -> Option<Arc<dyn RegisteredNativeVfsProvider>> {
    registry().get(shared_provider_id(path)?)
}

/// Resolve Core private storage through the same registration that owns the
/// provider identity. Callers select a provider capability, never an owner.
pub(crate) fn management_storage_for_provider(
    provider_id: &str,
    scope: &str,
) -> Result<fiex_storage::native::NativeScopedStorage, String> {
    registry().management_storage_for(provider_id, scope)
}

#[cfg(test)]
pub(crate) fn owner_for_provider(provider_id: &str) -> Option<CoreOwnerKey> {
    registry().owner(provider_id)
}

pub fn operations(path: &str) -> Vec<FsOperation> {
    let Some(provider) = view_for(path) else {
        return Vec::new();
    };
    let invocation = target_invocation(path, "core:capabilities", "vfs.discover", json!({}));
    provider.discover(&invocation).unwrap_or_default()
}

fn provider_error_text(error: ProviderError) -> String {
    if error.code == -32601 {
        format!("Unsupported: {}", error.message)
    } else {
        error.message
    }
}

pub fn require(
    path: &str,
    owner: &str,
    operation: FsOperation,
    create: bool,
) -> Result<(), String> {
    let provider = view_for(path).ok_or("Unsupported: unregistered native VFS provider")?;
    let invocation = target_invocation(path, owner, "vfs.discover", json!({}));
    provider
        .authorize(&invocation, operation, create)
        .map_err(provider_error_text)?;
    Ok(())
}

pub fn stat(path: &str, owner: &str) -> Result<Entry, String> {
    let provider = view_for(path).ok_or("Unsupported: unknown builtin node")?;
    let invocation = target_invocation(path, owner, "vfs.stat", json!({"nodeId":path}));
    let request = FsRequest::new(&invocation, FsOperation::Stat).map_err(provider_error_text)?;
    provider
        .stat(&request)
        .map(core_entry)
        .map_err(provider_error_text)
}

pub fn list(path: &str, owner: &str) -> Result<Vec<Entry>, String> {
    let provider = view_for(path).ok_or("Unsupported: builtin node has no children")?;
    let invocation = target_invocation(path, owner, "vfs.list", json!({"nodeId":path}));
    let request = FsRequest::new(&invocation, FsOperation::List).map_err(provider_error_text)?;
    provider
        .list(&request)
        .map(|listing| listing.entries.into_iter().map(core_entry).collect())
        .map_err(provider_error_text)
}

#[cfg(test)]
mod tests;
