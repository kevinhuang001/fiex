//! Native provider routing and provider-declared storage identities.
use super::{NativeVfsIdentity, registry};
use fiex_provider_contracts::ResourceType;

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
    provider.behavior().provider_identity(path, provider_id)
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
