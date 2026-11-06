//! Provider-specific callbacks attached during native VFS registration.
use super::{applications, recycle, trash_vfs, NativeVfsIdentity, APPLICATIONS_URI, RECENT_URI};
use crate::core::files::Entry;
use fiex_provider_contracts::ResourceType;
use std::path::{Path, PathBuf};

pub(super) fn recent_identity(_path: &str, provider_id: &'static str) -> Option<NativeVfsIdentity> {
    Some(NativeVfsIdentity {
        provider_id,
        storage_type: None,
        mount_id: provider_id.into(),
        available: true,
        host_path: false,
    })
}

pub(super) fn vfs_identity(_path: &str, provider_id: &'static str) -> Option<NativeVfsIdentity> {
    Some(NativeVfsIdentity {
        provider_id,
        storage_type: Some(ResourceType::Vfs),
        mount_id: provider_id.into(),
        available: true,
        host_path: false,
    })
}

pub(super) fn host_identity(_path: &str, provider_id: &'static str) -> Option<NativeVfsIdentity> {
    Some(NativeVfsIdentity {
        provider_id,
        storage_type: Some(ResourceType::Fs),
        mount_id: "local".into(),
        available: true,
        host_path: true,
    })
}

pub(super) fn usb_identity(path: &str, provider_id: &'static str) -> Option<NativeVfsIdentity> {
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

pub(super) fn validate_host_child(
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

pub(super) fn revalidate_recent_listing(entries: &mut Vec<Entry>) -> Result<(), String> {
    let storage = crate::core::volumes::native_vfs::management_storage_for_provider(
        fiex_recent::PROVIDER_ID,
        "history",
    )?;
    crate::core::recent::filter_cached_entries(entries, &storage)
}

#[cfg(feature = "desktop")]
pub(super) fn resolve_recent_navigation_source(alias: &str) -> Option<String> {
    crate::core::recent::source_for(alias).ok()
}

pub(super) fn trash_open_policy(_path: &str, entry: &Entry) -> Option<bool> {
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
pub(super) fn trash_open(_path: &str) -> Result<(), String> {
    Err(crate::i18n::t("回收站项目请使用“放回原处”或“彻底删除”").into())
}

/// Return a Core-local backing path only when the owning native provider
/// deliberately exposes one. Plugin mounts and projected virtual identities
/// never become host paths through this resolver.
pub(super) fn usb_ensure_current_binding(
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

pub(super) fn applications_is_tree_path(path: &str) -> bool {
    path == APPLICATIONS_URI
        || applications::is_folder(path)
        || applications::is_app(path)
        || applications::is_user(path)
}

pub(super) fn recent_is_tree_path(path: &str) -> bool {
    path == RECENT_URI
}

pub(super) fn recent_resource_kind(path: &str) -> &'static str {
    if path == RECENT_URI {
        "directory"
    } else {
        "file"
    }
}

pub(super) fn applications_resource_kind(path: &str) -> &'static str {
    if applications::is_folder(path) || path.ends_with('/') {
        "directory"
    } else {
        "file"
    }
}

pub(super) fn trash_resource_kind(path: &str) -> &'static str {
    if path == recycle::URI || path == trash_vfs::URI || path.ends_with('/') {
        "directory"
    } else {
        "file"
    }
}

pub(super) fn host_resource_kind(path: &str) -> &'static str {
    if path.ends_with('/') {
        "directory"
    } else {
        "file"
    }
}

pub(super) fn applications_icon(path: &str) -> Option<String> {
    if applications::is_user(path) {
        return None;
    }
    let entry = applications::system_entry(path).ok()?;
    applications::cached_icon(entry.link_target.as_deref()?)
}

pub(super) fn applications_default_view(_path: &str) -> Option<&'static str> {
    Some("icons")
}

pub(super) fn host_backing_path(path: &str) -> Option<PathBuf> {
    Some(PathBuf::from(path))
}

pub(super) fn validate_recent_child(parent: &str, child: &str) -> Result<(), String> {
    if parent != RECENT_URI {
        return Err("recent view has no writable child identities".into());
    }
    crate::core::recent::source_for(child).map(|_| ())
}

pub(super) fn host_child_path(
    parent: &str,
    name: &str,
    _kind: crate::core::fs::provider_registry::ChildKind,
) -> Result<String, String> {
    Ok(PathBuf::from(parent)
        .join(name)
        .to_string_lossy()
        .into_owned())
}

pub(super) fn usb_validate_child(parent: &str, child: &str) -> Result<(), String> {
    crate::usbfs::validate_child(parent, child)
}

pub(super) fn usb_child_path(
    parent: &str,
    name: &str,
    _kind: crate::core::fs::provider_registry::ChildKind,
) -> Result<String, String> {
    crate::usbfs::route(parent).ok_or("invalid USB parent")?;
    Ok(format!("{}/{}", parent.trim_end_matches(['/', '\\']), name))
}

pub(super) fn applications_child_path(
    parent: &str,
    name: &str,
    kind: crate::core::fs::provider_registry::ChildKind,
) -> Result<String, String> {
    let is_directory = matches!(kind, crate::core::fs::provider_registry::ChildKind::Folder);
    applications::reserve_child(parent, name, is_directory)
}
