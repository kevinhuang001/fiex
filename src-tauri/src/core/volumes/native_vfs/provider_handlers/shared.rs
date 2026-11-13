use crate::core::volumes::native_vfs::NativeVfsIdentity;
use fiex_provider_contracts::ResourceType;
use std::path::{Path, PathBuf};

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

pub(super) fn host_resource_kind(path: &str) -> &'static str {
    if path.ends_with('/') {
        "directory"
    } else {
        "file"
    }
}

pub(super) fn host_backing_path(path: &str) -> Option<PathBuf> {
    Some(PathBuf::from(path))
}
