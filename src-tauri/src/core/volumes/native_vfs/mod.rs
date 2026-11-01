//! Native storage providers implement the same shared VFS contracts as plugin providers.
use super::{applications, recycle, trash_vfs};
use crate::{
    core::files::Entry,
    plugins::provider_registry::{native::NativeProviderRegistry, FsOperation},
};
use fiex_provider_contracts::{
    FsEntry, FsListing, FsRead, FsRequest, FsWrite, ProviderError, ProviderInvocation, ProviderKind,
    ProviderMatchRule, SearchCapabilities, TransferDirection, TransferPreflight, VfsProvider,
};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};

mod application_provider;
mod recent_provider;
mod shared;
mod storage;
mod storage_provider;
mod trash_provider;

use application_provider::ApplicationView;
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

pub fn require(
    path: &str,
    owner: &str,
    operation: FsOperation,
    create: bool,
) -> Result<(), String> {
    let provider = view_for(path).ok_or("Unsupported: unregistered native VFS provider")?;
    if create {
        if provider_id(path) != Some("core:applications")
            || !applications::is_user(path)
            || !matches!(operation, FsOperation::Write | FsOperation::Mkdir)
        {
            return Err(format!("Unsupported: {}", operation.as_str()));
        }
        let backing = applications::candidate_path(path)?;
        let parent = backing.parent().ok_or("missing application parent")?;
        if parent.is_dir() && !backing.exists() {
            return Ok(());
        }
        return Err("Conflict: application destination is unavailable".into());
    }
    let invocation = target_invocation(path, owner, "vfs.discover", json!({}));
    provider
        .authorize(&invocation, operation, create)
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn stat(path: &str, owner: &str) -> Result<Entry, String> {
    let provider = view_for(path).ok_or("Unsupported: unknown builtin node")?;
    let invocation = target_invocation(path, owner, "vfs.stat", json!({"nodeId":path}));
    let request =
        FsRequest::new(&invocation, FsOperation::Stat).map_err(|error| error.to_string())?;
    provider
        .stat(&request)
        .map(core_entry)
        .map_err(|error| error.to_string())
}

pub fn list(path: &str, owner: &str) -> Result<Vec<Entry>, String> {
    let provider = view_for(path).ok_or("Unsupported: builtin node has no children")?;
    let invocation = target_invocation(path, owner, "vfs.list", json!({"nodeId":path}));
    let request =
        FsRequest::new(&invocation, FsOperation::List).map_err(|error| error.to_string())?;
    provider
        .list(&request)
        .map(|listing| listing.entries.into_iter().map(core_entry).collect())
        .map_err(|error| error.to_string())
}

fn basename(path: &str) -> Result<String, String> {
    let path = if applications::is_user(path) {
        applications::candidate_path(path)?
    } else {
        PathBuf::from(path)
    };
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| "InvalidVfsUri: node has no name".into())
}

pub fn stat_for(broker: &crate::core::fs::ResourceBroker) -> Result<Entry, String> {
    let provider =
        shared_view_for(&broker.path).ok_or("Unsupported: unregistered native VFS provider")?;
    let invocation = shared::broker_invocation(
        broker,
        None,
        "vfs.stat",
        json!({"nodeId":broker.path.as_str()}),
    );
    let request =
        FsRequest::new(&invocation, FsOperation::Stat).map_err(|error| error.to_string())?;
    provider
        .stat(&request)
        .map(core_entry)
        .map_err(|error| error.to_string())
}

pub fn list_for(broker: &crate::core::fs::ResourceBroker) -> Result<Vec<Entry>, String> {
    let provider =
        shared_view_for(&broker.path).ok_or("Unsupported: unregistered native VFS provider")?;
    let invocation = shared::broker_invocation(
        broker,
        None,
        "vfs.list",
        json!({"nodeId":broker.path.as_str(),"hidden":true}),
    );
    let request =
        FsRequest::new(&invocation, FsOperation::List).map_err(|error| error.to_string())?;
    provider
        .list(&request)
        .map(|listing| listing.entries.into_iter().map(core_entry).collect())
        .map_err(|error| error.to_string())
}

pub fn require_for(
    broker: &crate::core::fs::ResourceBroker,
    operation: FsOperation,
    create: bool,
) -> Result<(), String> {
    if create && provider_id(&broker.path) == Some("core:applications") {
        return require(&broker.path, &broker.owner, operation, true);
    }
    let provider =
        shared_view_for(&broker.path).ok_or("Unsupported: unregistered native VFS provider")?;
    let invocation = shared::broker_invocation(
        broker,
        None,
        "vfs.discover",
        json!({"operation":operation.as_str(),"create":create}),
    );
    provider
        .authorize(&invocation, operation, create)
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn preflight_for(
    broker: &crate::core::fs::ResourceBroker,
    request: &TransferPreflight,
) -> Result<(), String> {
    let provider =
        shared_view_for(&broker.path).ok_or("Unsupported: unregistered native VFS provider")?;
    let invocation = shared::broker_invocation(broker, None, "vfs.preflight", json!({}));
    let result = provider
        .preflight(&invocation, request)
        .map_err(|error| error.to_string())?;
    if result.allowed {
        Ok(())
    } else {
        Err(result
            .reason
            .unwrap_or_else(|| "PermissionDenied: transfer denied by provider".into()))
    }
}

pub fn read_for(broker: &crate::core::fs::ResourceBroker) -> Result<FsRead, String> {
    let provider =
        shared_view_for(&broker.path).ok_or("Unsupported: unregistered native VFS provider")?;
    let invocation = shared::broker_invocation(
        broker,
        None,
        "vfs.openRead",
        json!({"nodeId":broker.path.as_str()}),
    );
    let request =
        FsRequest::new(&invocation, FsOperation::Read).map_err(|error| error.to_string())?;
    provider.read(&request).map_err(|error| error.to_string())
}

pub fn write_sized_for(
    broker: &crate::core::fs::ResourceBroker,
    create: bool,
    size: Option<u64>,
) -> Result<FsWrite, String> {
    let provider =
        shared_view_for(&broker.path).ok_or("Unsupported: unregistered native VFS provider")?;
    let params = if create {
        let parent = crate::core::fs::parent_path(&broker.path)?;
        let name = basename(&broker.path)?;
        json!({"parentId":parent,"name":name,"size":size})
    } else {
        json!({"nodeId":broker.path.as_str(),"size":size})
    };
    let invocation = shared::broker_invocation(broker, None, "vfs.beginWrite", params);
    let request =
        FsRequest::new(&invocation, FsOperation::Write).map_err(|error| error.to_string())?;
    provider.write(&request).map_err(|error| error.to_string())
}

pub fn mkdir_for(broker: &crate::core::fs::ResourceBroker) -> Result<FsEntry, String> {
    let provider =
        shared_view_for(&broker.path).ok_or("Unsupported: unregistered native VFS provider")?;
    let parent = crate::core::fs::parent_path(&broker.path)?;
    let name = basename(&broker.path)?;
    let invocation = shared::broker_invocation(
        broker,
        None,
        "vfs.mkdir",
        json!({"parentId":parent,"name":name}),
    );
    let request =
        FsRequest::new(&invocation, FsOperation::Mkdir).map_err(|error| error.to_string())?;
    provider.mkdir(&request).map_err(|error| error.to_string())
}

pub fn rename_for(
    source: &crate::core::fs::ResourceBroker,
    destination: &crate::core::fs::ResourceBroker,
) -> Result<FsEntry, String> {
    let source_id =
        shared_provider_id(&source.path).ok_or("Unsupported: unregistered native VFS provider")?;
    if Some(source_id) != shared_provider_id(&destination.path) {
        return Err("Unsupported: cross-provider rename requires TransferEngine".into());
    }
    let provider = registry()
        .get(source_id)
        .ok_or("Unsupported: unregistered native VFS provider")?;
    let parent = crate::core::fs::parent_path(&destination.path)?;
    let name = basename(&destination.path)?;
    let invocation = shared::broker_invocation(
        source,
        Some(destination),
        "vfs.rename",
        json!({"nodeId":source.path.as_str(),"parentId":parent,"name":name}),
    );
    let request =
        FsRequest::new(&invocation, FsOperation::Rename).map_err(|error| error.to_string())?;
    provider.rename(&request).map_err(|error| error.to_string())
}

pub fn remove_for(broker: &crate::core::fs::ResourceBroker) -> Result<(), String> {
    let provider =
        shared_view_for(&broker.path).ok_or("Unsupported: unregistered native VFS provider")?;
    let invocation = shared::broker_invocation(
        broker,
        None,
        "vfs.remove",
        json!({"nodeId":broker.path.as_str()}),
    );
    let request =
        FsRequest::new(&invocation, FsOperation::Remove).map_err(|error| error.to_string())?;
    provider.remove(&request).map_err(|error| error.to_string())
}

pub fn search_for(
    broker: &crate::core::fs::ResourceBroker,
    query: &str,
    hidden: bool,
    ignores: &[String],
    limit: usize,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<Entry>, bool), String> {
    let provider =
        shared_view_for(&broker.path).ok_or("Unsupported: unregistered native VFS provider")?;
    let invocation = shared::broker_invocation(
        broker,
        None,
        "vfs.search",
        json!({"nodeId":broker.path.as_str(),"query":query,"hidden":hidden,"ignores":ignores,"limit":limit}),
    );
    let request =
        FsRequest::new(&invocation, FsOperation::Search).map_err(|error| error.to_string())?;
    let listing = provider
        .search_cancellable(&request, cancelled)
        .map_err(|error| error.to_string())?;
    Ok((
        listing.entries.into_iter().map(core_entry).collect(),
        listing.truncated,
    ))
}

/// URI-backed resources have no host path. Never canonicalize or materialize
/// these identifiers as files, even for metadata-only extension calls.
pub fn opaque(path: &str) -> bool {
    provider_id(path).is_some()
}

pub fn resource_entry(path: &str) -> Result<Entry, String> {
    stat(path, "core:resource-metadata")
}

pub fn icon_for(path: &str) -> Option<String> {
    if provider_id(path) != Some("core:applications") || applications::is_user(path) {
        return None;
    }
    let entry = applications::system_entry(path).ok()?;
    applications::cached_icon(entry.link_target.as_deref()?)
}

pub fn default_view(path: &str) -> Option<&'static str> {
    (provider_id(path) == Some("core:applications")).then_some("icons")
}

pub fn readonly(path: &str) -> bool {
    resource_entry(path)
        .map(|entry| entry.readonly)
        .unwrap_or(true)
}

/// A Recent alias can be revealed only when its original resource still exists
/// on a local filesystem. Remote, USBFS and plugin-mount sources are excluded.
pub(crate) fn recent_reveal_source(alias: &str) -> Option<String> {
    let source = crate::core::recent::source_for(alias).ok()?;
    revealable_source(&source).then_some(source)
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
