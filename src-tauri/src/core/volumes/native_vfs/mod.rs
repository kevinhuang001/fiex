//! Native storage providers implement the shared VFS contracts used by Core and plugins.
use super::{applications, recycle, trash_vfs};
#[allow(unused_imports)] // Native VFS adapters still share this internal module scope.
use crate::{core::files::Entry, plugins::provider_registry::native::NativeProviderRegistry};
#[allow(unused_imports)] // Native VFS adapters still share this internal module scope.
use fiex_provider_contracts::{
    FsEntry, FsListing, FsOperation, FsRead, FsRequest, FsWrite, PreflightHandlers, ProviderError,
    ProviderInvocation, ProviderMatchRule, SearchCapabilities, VfsProvider,
};
#[cfg(test)]
use fiex_provider_contracts::{ProviderKind, ResourceType};
#[cfg(test)]
use fiex_storage::CoreOwnerKey;
use serde_json::json;
#[allow(unused_imports)] // Native VFS adapters still share this internal module scope.
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

mod application_provider;
mod broker;
mod extensions;
mod identity;
mod presentation;
mod provider_handlers;
mod recent_provider;
mod registration;
mod shared;
mod storage;
mod storage_provider;
mod trash_provider;

pub use broker::{
    list_for, mkdir_for, preflight_for, read_for, remove_for, rename_for, require_for, search_for,
    stat_for, trash_for, write_sized_for,
};
pub(crate) use extensions::shared_view_for;
pub(crate) use extensions::{
    after_rename, backing_path, child_path, ensure_current_binding, move_within_mount, open_policy,
    revalidate_cached_listing, validate_child_with_scope, view_for,
};
pub use extensions::{is_tree_path, parent_path, tree_path_key, validate_child};
#[cfg(feature = "desktop")]
pub(crate) use extensions::{navigation_source, open};
pub(crate) use identity::{identity, is_filesystem_path, is_host_path, reference_mount_id};
pub use identity::{provider_id, shared_provider_id};
#[cfg(test)]
pub(crate) use presentation::navigable_source;
pub(crate) use presentation::recent_navigation_source;
pub use presentation::{default_view, icon_for, opaque, resource_entry};
pub(crate) use registration::{NativeVfsBehavior, NativeVfsIdentity};
pub(crate) use registration::{RegisteredNativeVfsProvider, register, registry};
pub use shared::core_entry;
use shared::target_invocation;

pub const RECENT_URI: &str = "recent://";
pub const APPLICATIONS_URI: &str = applications::URI;

pub(crate) fn recent_services() -> Arc<dyn fiex_recent::RecentModuleServices> {
    recent_provider::services()
}

pub(crate) fn recent_source_for(alias: &str) -> Result<String, String> {
    recent_provider::source_for(alias)
}

pub(crate) fn filter_recent_cached_entries(
    entries: &mut Vec<crate::core::files::Entry>,
) -> Result<(), String> {
    let storage = management_storage_for_provider(fiex_recent::PROVIDER_ID, "history")?;
    recent_provider::filter_cached_entries(entries, &storage)
}

pub(crate) fn trash_services() -> Arc<dyn fiex_trash::TrashModuleServices> {
    trash_provider::services()
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
