use super::shared::{host_backing_path, host_identity, host_resource_kind, usb_identity};
use crate::core::volumes::native_vfs::{NativeVfsBehavior, NativeVfsIdentity};
use std::path::PathBuf;

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

pub(crate) struct LocalStorageOperations;

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
        Some(super::shared::validate_host_child(scope, parent, child))
    }
}

pub(crate) struct UsbStorageOperations;

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
