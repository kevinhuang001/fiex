//! Dispatch optional native provider behavior through the registered instance.
use super::{
    identity::{provider_id, shared_provider_id},
    opaque, registry, RegisteredNativeVfsProvider,
};
use crate::core::files::Entry;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

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

pub fn tree_path_key(path: &str) -> Option<String> {
    is_tree_path(path).then(|| path.to_owned())
}

pub(crate) fn view_for(path: &str) -> Option<Arc<dyn RegisteredNativeVfsProvider>> {
    registry().get(provider_id(path)?)
}

pub(crate) fn shared_view_for(path: &str) -> Option<Arc<dyn RegisteredNativeVfsProvider>> {
    registry().get(shared_provider_id(path)?)
}
