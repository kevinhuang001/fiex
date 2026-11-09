//! State and policy for read-only filesystem-driver fallbacks.
#[cfg(feature = "usbfs")]
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

#[cfg(feature = "usbfs")]
#[derive(Clone)]
pub(super) struct FallbackMount {
    pub(super) path: String,
    pub(super) eject_device: String,
}

#[cfg(feature = "usbfs")]
fn usb_fallbacks() -> &'static Mutex<HashMap<String, FallbackMount>> {
    static FALLBACKS: OnceLock<Mutex<HashMap<String, FallbackMount>>> = OnceLock::new();
    FALLBACKS.get_or_init(Default::default)
}

/// Clear the fallback-provider record after ejecting or retrying a native mount.
pub fn clear_usb_fallback(device: &str) {
    #[cfg(feature = "usbfs")]
    if let Ok(mut fallbacks) = usb_fallbacks().lock() {
        fallbacks.remove(device);
    }
    #[cfg(not(feature = "usbfs"))]
    let _ = device;
}

/// The fallback view is used only after the native OS mount reports that its
/// filesystem driver is unavailable. The provider is intentionally limited to
/// Btrfs and Apple formats; local OS mounts own all other USB filesystems.
#[cfg(feature = "usbfs")]
pub(super) fn fallback_mount(
    device_key: &str,
    provider_device: &str,
    eject_device: &str,
    filesystem: &str,
) -> Option<Result<String, String>> {
    let kind = crate::usbfs::detect(filesystem)
        .or_else(|| crate::usbfs::probe(provider_device))
        .filter(|kind| {
            matches!(
                kind,
                crate::usbfs::FsKind::Apfs
                    | crate::usbfs::FsKind::Hfs
                    | crate::usbfs::FsKind::Btrfs
            )
        })?;
    let path = crate::usbfs::virtual_root(provider_device, kind);
    let route =
        crate::usbfs::route(&path).ok_or_else(|| "invalid read-only USB fallback".to_string());
    Some(route.and_then(|route| {
        crate::usbfs::ensure_access(&route)?;
        usb_fallbacks()
            .lock()
            .map_err(|_| "USB fallback registry unavailable".to_string())?
            .insert(
                device_key.into(),
                FallbackMount {
                    path: path.clone(),
                    eject_device: eject_device.into(),
                },
            );
        Ok(path)
    }))
}

#[cfg(feature = "usbfs")]
pub(super) fn active_fallback(device: &str) -> Option<FallbackMount> {
    usb_fallbacks().lock().ok()?.get(device).cloned()
}

#[cfg(not(feature = "usbfs"))]
pub(super) fn active_fallback(_device: &str) -> Option<()> {
    None
}

pub fn usb_fallback_eject_device(device: &str) -> Option<String> {
    #[cfg(feature = "usbfs")]
    return active_fallback(device).map(|mount| mount.eject_device);
    #[cfg(not(feature = "usbfs"))]
    None
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(super) fn native_mount_unsupported(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    // Linux may report an unsupported filesystem from the unprivileged mount
    // attempt, then report an authorization failure from pkexec. The latter
    // must win: permission or user-cancellation errors must never open the raw
    // fallback provider.
    if [
        "permission denied",
        "operation not permitted",
        "access denied",
        "not authorized",
        "not authorised",
        "authentication is required",
        "authentication failed",
        "authentication canceled",
        "authentication cancelled",
        "cancelled by user",
        "canceled by user",
    ]
    .iter()
    .any(|phrase| error.contains(phrase))
    {
        return false;
    }
    [
        "unknown filesystem",
        "unsupported filesystem",
        "filesystem not supported",
        "not supported by this system",
        "no mountable file systems",
        "unknown file system",
        "file system not recognized",
    ]
    .iter()
    .any(|phrase| error.contains(phrase))
}

#[cfg(windows)]
pub(super) fn windows_native_mount_unsupported(error: &std::io::Error) -> bool {
    // Windows filesystem discovery returns these errors when the volume has
    // no installed filesystem driver. Access denied is deliberately excluded:
    // ACL and device permission failures belong to Error Center.
    matches!(error.raw_os_error(), Some(1 | 50 | 1005 | 1112))
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(super) fn fallback_after_native_failure(
    device_key: &str,
    provider_device: &str,
    eject_device: &str,
    filesystem: &str,
    error: &str,
) -> Option<Result<String, String>> {
    if !native_mount_unsupported(error) {
        return None;
    }
    #[cfg(feature = "usbfs")]
    return fallback_mount(device_key, provider_device, eject_device, filesystem);
    #[cfg(not(feature = "usbfs"))]
    None
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(super) fn virtual_path(device: &str, filesystem: &str) -> Option<String> {
    let _ = filesystem;
    #[cfg(feature = "usbfs")]
    return active_fallback(device).map(|mount| mount.path);
    #[cfg(not(feature = "usbfs"))]
    None
}
