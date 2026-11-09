//! Provider-specific callbacks attached during native VFS registration.
use super::{
    APPLICATIONS_URI, NativeVfsBehavior, NativeVfsIdentity, RECENT_URI, application_provider,
    applications, recycle, trash_vfs,
};
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
    crate::core::volumes::native_vfs::filter_recent_cached_entries(entries)
}

#[cfg(feature = "desktop")]
pub(super) fn resolve_recent_navigation_source(alias: &str) -> Option<String> {
    crate::core::volumes::native_vfs::recent_source_for(alias).ok()
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
    crate::core::volumes::native_vfs::recent_source_for(child).map(|_| ())
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
    match kind {
        crate::core::fs::provider_registry::ChildKind::Folder => {
            applications::reserve_child(parent, name, true)
        }
        crate::core::fs::provider_registry::ChildKind::Application => {
            applications::reserve_child(parent, name, false)
        }
        crate::core::fs::provider_registry::ChildKind::File => {
            Err("Applications does not support creating ordinary files".into())
        }
    }
}

pub(super) struct RecentOperations;

impl NativeVfsBehavior for RecentOperations {
    fn validate_child(&self, parent: &str, child: &str) -> Option<Result<(), String>> {
        Some(validate_recent_child(parent, child))
    }

    fn is_tree_path(&self, path: &str) -> bool {
        recent_is_tree_path(path)
    }

    fn resource_kind(&self, path: &str) -> Option<&'static str> {
        Some(recent_resource_kind(path))
    }

    fn provider_identity(
        &self,
        path: &str,
        provider_id: &'static str,
    ) -> Option<NativeVfsIdentity> {
        recent_identity(path, provider_id)
    }

    #[cfg(feature = "desktop")]
    fn navigation_source(&self, path: &str) -> Option<String> {
        resolve_recent_navigation_source(path)
    }

    fn revalidate_cached_listing(&self, entries: &mut Vec<Entry>) -> Option<Result<(), String>> {
        Some(revalidate_recent_listing(entries))
    }
}

pub(super) struct ApplicationsOperations;

impl NativeVfsBehavior for ApplicationsOperations {
    #[cfg(feature = "desktop")]
    fn open(&self, path: &str) -> Option<Result<(), String>> {
        Some(applications::open(path))
    }

    fn validate_child(&self, parent: &str, child: &str) -> Option<Result<(), String>> {
        Some(applications::validate_child(parent, child))
    }

    fn child_path(
        &self,
        parent: &str,
        name: &str,
        kind: crate::core::fs::provider_registry::ChildKind,
    ) -> Option<Result<String, String>> {
        Some(applications_child_path(parent, name, kind))
    }

    fn parent_path(&self, path: &str) -> Option<Result<String, String>> {
        Some(applications::parent_uri(path))
    }

    fn same_mount_move(
        &self,
        sources: &[crate::core::fs::ResourceBroker],
        destination: &crate::core::fs::ResourceBroker,
    ) -> Option<Result<Vec<String>, String>> {
        Some(application_provider::move_within_mount(
            sources,
            destination,
        ))
    }

    fn backing_path(&self, path: &str) -> Option<PathBuf> {
        application_provider::backing_path(path)
    }

    fn after_rename(&self, path: &str) {
        applications::cancel_reservation(path);
    }

    fn is_tree_path(&self, path: &str) -> bool {
        applications_is_tree_path(path)
    }

    fn resource_kind(&self, path: &str) -> Option<&'static str> {
        Some(applications_resource_kind(path))
    }

    fn icon(&self, path: &str) -> Option<String> {
        applications_icon(path)
    }

    fn default_view(&self, path: &str) -> Option<&'static str> {
        applications_default_view(path)
    }

    fn provider_identity(
        &self,
        path: &str,
        provider_id: &'static str,
    ) -> Option<NativeVfsIdentity> {
        vfs_identity(path, provider_id)
    }
}

pub(super) struct TrashOperations;

impl NativeVfsBehavior for TrashOperations {
    fn validate_child(&self, parent: &str, child: &str) -> Option<Result<(), String>> {
        Some(trash_vfs::validate_child(parent, child))
    }

    fn is_tree_path(&self, path: &str) -> bool {
        trash_vfs::is_tree(path)
    }

    fn resource_kind(&self, path: &str) -> Option<&'static str> {
        Some(trash_resource_kind(path))
    }

    fn parent_path(&self, path: &str) -> Option<Result<String, String>> {
        Some(trash_vfs::parent_path(path))
    }

    fn provider_identity(
        &self,
        path: &str,
        provider_id: &'static str,
    ) -> Option<NativeVfsIdentity> {
        vfs_identity(path, provider_id)
    }

    #[cfg(feature = "desktop")]
    fn open(&self, path: &str) -> Option<Result<(), String>> {
        Some(trash_open(path))
    }

    fn open_policy(&self, path: &str, entry: &Entry) -> Option<bool> {
        trash_open_policy(path, entry)
    }
}

pub(super) struct LocalStorageOperations;

impl NativeVfsBehavior for LocalStorageOperations {
    fn child_path(
        &self,
        parent: &str,
        name: &str,
        kind: crate::core::fs::provider_registry::ChildKind,
    ) -> Option<Result<String, String>> {
        Some(host_child_path(parent, name, kind))
    }

    fn backing_path(&self, path: &str) -> Option<PathBuf> {
        host_backing_path(path)
    }

    fn provider_identity(
        &self,
        path: &str,
        provider_id: &'static str,
    ) -> Option<NativeVfsIdentity> {
        host_identity(path, provider_id)
    }

    fn resource_kind(&self, path: &str) -> Option<&'static str> {
        Some(host_resource_kind(path))
    }

    fn validate_scoped_child(
        &self,
        scope: &crate::plugins::resource::fs_access::Scope,
        parent: &str,
        child: &str,
    ) -> Option<Result<(), String>> {
        Some(validate_host_child(scope, parent, child))
    }
}

pub(super) struct UsbStorageOperations;

impl NativeVfsBehavior for UsbStorageOperations {
    fn validate_child(&self, parent: &str, child: &str) -> Option<Result<(), String>> {
        Some(usb_validate_child(parent, child))
    }

    fn child_path(
        &self,
        parent: &str,
        name: &str,
        kind: crate::core::fs::provider_registry::ChildKind,
    ) -> Option<Result<String, String>> {
        Some(usb_child_path(parent, name, kind))
    }

    fn validate_binding(
        &self,
        path: &str,
        reference: &crate::plugins::contracts::ResourceRef,
    ) -> Option<Result<(), String>> {
        Some(usb_ensure_current_binding(path, reference))
    }

    fn provider_identity(
        &self,
        path: &str,
        provider_id: &'static str,
    ) -> Option<NativeVfsIdentity> {
        usb_identity(path, provider_id)
    }

    fn resource_kind(&self, path: &str) -> Option<&'static str> {
        Some(host_resource_kind(path))
    }
}
