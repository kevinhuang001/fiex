//! Native storage providers implement the same shared VFS contracts as plugin providers.
use super::{applications, recycle, trash_vfs};
use crate::{
    core::files::Entry,
    plugins::provider_registry::{native::NativeProviderRegistry, FsOperation},
};
use fiex_provider_contracts::{
    FsEntry, FsListing, FsRead, FsRequest, FsWrite, ProviderError, ProviderInvocation,
    ProviderKind, ProviderMatchRule, SearchCapabilities, TransferDirection, TransferPreflight,
    VfsProvider,
};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
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
    stat_for, write_sized_for,
};
pub(crate) use presentation::recent_reveal_source;
pub use presentation::{default_view, icon_for, opaque, resource_entry};
use recent_provider::RecentView;
pub use shared::core_entry;
use shared::target_invocation;
use storage_provider::StorageProvider;
use trash_provider::TrashView;

pub const RECENT_URI: &str = "recent://";
pub const APPLICATIONS_URI: &str = applications::URI;

/// A projected alias carries a view identity and resolves its storage owner separately.
pub enum NativeStorageProjection {
    Owned,
    Projected(Option<String>),
}

fn registry() -> &'static NativeProviderRegistry<dyn VfsProvider> {
    static REGISTERED: OnceLock<NativeProviderRegistry<dyn VfsProvider>> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let mut registry: NativeProviderRegistry<dyn VfsProvider> =
            NativeProviderRegistry::new(ProviderKind::Vfs);
        for (id, provider) in [
            ("core:recent", Arc::new(RecentView) as Arc<dyn VfsProvider>),
            (
                "core:applications",
                Arc::new(ApplicationView) as Arc<dyn VfsProvider>,
            ),
            ("core:trash", Arc::new(TrashView) as Arc<dyn VfsProvider>),
            (
                "core:local-fs",
                Arc::new(StorageProvider::new("core:local-fs")) as Arc<dyn VfsProvider>,
            ),
            (
                "core:usb-fs",
                Arc::new(StorageProvider::new("core:usb-fs")) as Arc<dyn VfsProvider>,
            ),
        ] {
            registry
                .register(id, 100, ProviderMatchRule::default(), provider)
                .expect("invalid native VFS registration");
        }
        registry
    })
}

/// Resolve the native VFS identity at the URI boundary. General services use
/// the returned descriptor ID and shared provider instance thereafter.
pub fn provider_id(path: &str) -> Option<&'static str> {
    if path.starts_with(RECENT_URI) {
        Some("core:recent")
    } else if path.starts_with(APPLICATIONS_URI) {
        Some("core:applications")
    } else if recycle::is_virtual(path) || trash_vfs::handles(path) {
        Some("core:trash")
    } else {
        None
    }
}

/// Resolve a Core storage provider without promoting unknown URI schemes to
/// local paths. Local and USB resources use the same shared trait registry as
/// the built-in virtual views.
pub fn shared_provider_id(path: &str) -> Option<&'static str> {
    if let Some(provider) = provider_id(path) {
        return Some(provider);
    }
    if crate::usbfs::route(path).is_some() {
        return Some("core:usb-fs");
    }
    if path.is_empty()
        || path.contains("://")
        || crate::plugins::mounts::handles(path)
        || unregistered_scheme(path)
    {
        return None;
    }
    Some("core:local-fs")
}

/// Return a Core-local backing path only when the owning native provider
/// deliberately exposes one. Plugin mounts and projected virtual identities
/// never become host paths through this resolver.
pub fn backing_path(path: &str) -> Option<PathBuf> {
    match shared_provider_id(path)? {
        "core:local-fs" => Some(PathBuf::from(path)),
        "core:applications" => application_provider::backing_path(path),
        _ => None,
    }
}

/// USB references are scoped to a mount generation. Keep that liveness check
/// at the native storage boundary so generic ResourceBroker code stays free
/// of USB-specific policy.
pub(crate) fn ensure_current_binding(
    path: &str,
    reference: &crate::plugins::contracts::ResourceRef,
) -> Result<(), String> {
    if shared_provider_id(path) != Some("core:usb-fs") {
        return Ok(());
    }
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
    path == RECENT_URI
        || path == APPLICATIONS_URI
        || applications::is_user(path)
        || trash_vfs::is_tree(path)
}

/// Ask the owning native view to validate a child identity. Ordinary host paths
/// are validated by the enclosing operation scope in Core's VFS router.
pub fn validate_child(parent: &str, child: &str) -> Result<(), String> {
    if crate::usbfs::route(parent).is_some() {
        return crate::usbfs::validate_child(parent, child);
    }
    match provider_id(parent) {
        Some("core:recent") if parent == RECENT_URI => {
            crate::core::recent::source_for(child).map(|_| ())
        }
        Some("core:applications") => applications::validate_child(parent, child),
        Some("core:trash") => trash_vfs::validate_child(parent, child),
        _ => Err("Unsupported: provider does not support child identities".into()),
    }
}

pub fn child_path(parent: &str, name: &str) -> Result<String, String> {
    match shared_provider_id(parent) {
        Some("core:local-fs") => Ok(PathBuf::from(parent)
            .join(name)
            .to_string_lossy()
            .into_owned()),
        Some("core:usb-fs") => {
            crate::usbfs::route(parent).ok_or("invalid USB parent")?;
            Ok(format!("{}/{}", parent.trim_end_matches(['/', '\\']), name))
        }
        Some("core:applications") => {
            let backing = if parent == APPLICATIONS_URI {
                applications::stored_path(parent)?
            } else {
                applications::candidate_path(parent)?
            };
            applications::stored_uri(&backing.join(name))
        }
        _ => Err("Unsupported: provider does not support child creation".into()),
    }
}

pub fn parent_path(path: &str) -> Result<String, String> {
    if applications::is_user(path) {
        let local = applications::candidate_path(path)?;
        let parent = local.parent().ok_or("item has no parent")?;
        if parent == applications::stored_path(applications::URI)? {
            return Ok(applications::URI.into());
        }
        return applications::stored_uri(parent);
    }
    if opaque(path) {
        return Err("view has no writable parent".into());
    }
    Path::new(path)
        .parent()
        .map(|path| path.to_string_lossy().into_owned())
        .ok_or_else(|| "item has no parent".into())
}

pub fn tree_path_key(path: &str) -> Option<String> {
    is_tree_path(path).then(|| path.to_owned())
}

pub fn view_for(path: &str) -> Option<&'static dyn VfsProvider> {
    registry().get(provider_id(path)?)
}

fn shared_view_for(path: &str) -> Option<&'static dyn VfsProvider> {
    registry().get(shared_provider_id(path)?)
}

pub fn storage_projection(path: &str) -> NativeStorageProjection {
    if provider_id(path) == Some("core:recent") {
        NativeStorageProjection::Projected(crate::core::recent::source_for(path).ok())
    } else {
        NativeStorageProjection::Owned
    }
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

fn revealable_source(source: &str) -> bool {
    let path = std::path::Path::new(source);
    !crate::remote::is_remote(source)
        && !crate::plugins::mounts::handles(source)
        && crate::usbfs::route(source).is_none()
        && !opaque(source)
        && path.is_absolute()
        && path.is_file()
}

#[cfg(test)]
mod tests;
