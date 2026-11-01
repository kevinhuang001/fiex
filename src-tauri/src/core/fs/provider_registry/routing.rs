//! Provider ownership and resource identity routing.
use super::FsError;
use crate::{core::volumes::native_vfs, plugins::mounts, plugins::resource::fs_access::Scope};

pub(super) enum Route {
    Plugin,
    NativeProvider,
}

/// Resource access is derived from the live storage registry. Higher layers
/// need only know whether a resource has a host path or must stay behind its
/// registered provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResourceAccess {
    HostPath,
    Provider,
}

pub(crate) enum ResourceOwner {
    LocalFs,
    PluginMount {
        plugin_id: String,
        mount_id: String,
    },
    NativeVfs(&'static str),
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

pub(super) fn resolve(path: &str) -> Result<Route, FsError> {
    if mounts::handles(path) {
        return Ok(Route::Plugin);
    }
    if native_vfs::shared_provider_id(path).is_some() {
        return Ok(Route::NativeProvider);
    }
    if path.is_empty() || path.contains("://") || unregistered_scheme(path) {
        return Err("Unsupported: unregistered filesystem URI".into());
    }
    Err("Unsupported: unregistered filesystem resource".into())
}

pub(crate) fn owner(path: &str) -> Result<ResourceOwner, FsError> {
    match resolve(path)? {
        Route::Plugin => {
            let (uri, source) = mounts::resolve(path).map_err(FsError::from)?;
            let plugin_id = match source {
                mounts::Source::Connection(connection) => connection.plugin_id,
                mounts::Source::View(view) => view.descriptor.plugin_id,
            };
            Ok(ResourceOwner::PluginMount {
                plugin_id,
                mount_id: uri.mount_id,
            })
        }
        Route::NativeProvider => match native_vfs::shared_provider_id(path) {
            Some("core:local-fs") => Ok(ResourceOwner::LocalFs),
            Some(provider) => Ok(ResourceOwner::NativeVfs(provider)),
            None => Err("Unsupported: resource owner is no longer registered".into()),
        },
    }
}

pub(crate) fn resource_access(path: &str) -> Result<ResourceAccess, FsError> {
    match owner(path)? {
        ResourceOwner::LocalFs => Ok(ResourceAccess::HostPath),
        ResourceOwner::PluginMount { .. } | ResourceOwner::NativeVfs(_) => {
            Ok(ResourceAccess::Provider)
        }
    }
}

pub(crate) fn is_plugin_mount(path: &str) -> bool {
    matches!(resolve(path), Ok(Route::Plugin))
}

/// Validate child ownership at the storage-provider boundary. Resource scopes
/// carry authorization state; concrete mount semantics stay with their owner.
pub(crate) fn validate_child(
    scope: &Scope,
    parent: &str,
    child: &str,
) -> Result<(), String> {
    match resolve(parent).map_err(String::from)? {
        Route::Plugin => super::plugin::validate_child(parent, child),
        Route::NativeProvider => match native_vfs::shared_provider_id(parent) {
            Some("core:local-fs") => {
                scope.authorize(child)?;
                if !std::path::Path::new(child).starts_with(parent) {
                    return Err("child outside selected resource".into());
                }
                Ok(())
            }
            Some(_) => native_vfs::validate_child(parent, child),
            None => Err("Unsupported: resource owner is no longer registered".into()),
        },
    }
}

pub(crate) fn child_path(parent: &str, name: &str) -> Result<String, String> {
    crate::plugins::mounts::segment(name)?;
    match resolve(parent).map_err(String::from)? {
        Route::Plugin => super::plugin::child_path(parent, name),
        Route::NativeProvider => native_vfs::child_path(parent, name),
    }
}

pub(crate) fn parent_path(path: &str) -> Result<String, String> {
    match resolve(path).map_err(String::from)? {
        Route::Plugin => super::plugin::parent_path(path),
        Route::NativeProvider => native_vfs::parent_path(path),
    }
}

/// The same Core-owned storage identity drives operations and resource matching.
pub(crate) fn identity(path: &str) -> Option<(&'static str, String, String)> {
    match resolve(path).ok()? {
        Route::Plugin => {
            let (uri, source) = mounts::resolve(path).ok()?;
            let provider = match source {
                mounts::Source::Connection(connection) => {
                    format!("plugin:{}:{}", connection.plugin_id, connection.kind)
                }
                mounts::Source::View(view) => format!(
                    "plugin:{}:{}",
                    view.descriptor.plugin_id, view.descriptor.contribution_id
                ),
            };
            Some(("vfs", provider, uri.mount_id))
        }
        Route::NativeProvider => {
            let provider = native_vfs::shared_provider_id(path)?;
            if provider == "core:local-fs" {
                return Some(("fs", provider.into(), "local".into()));
            }
            if provider == "core:usb-fs" {
                let route = crate::usbfs::route(path)?;
                let generation =
                    crate::usbfs::generation(&route).unwrap_or_else(|_| "offline".into());
                return Some(("fs", provider.into(), format!("core:usb:{generation}")));
            }
            if !matches!(
                native_vfs::storage_projection(path),
                native_vfs::NativeStorageProjection::Owned
            ) {
                return None;
            }
            Some(("vfs", provider.into(), provider.into()))
        }
    }
}

pub(crate) fn reference_mount_id(path: &str) -> Option<String> {
    match resolve(path).ok()? {
        Route::Plugin => mounts::parse(path).ok().map(|uri| uri.mount_id),
        Route::NativeProvider => match native_vfs::shared_provider_id(path)? {
            "core:local-fs" => Some("local".into()),
            "core:usb-fs" => {
                let route = crate::usbfs::route(path)?;
                let generation =
                    crate::usbfs::generation(&route).unwrap_or_else(|_| "offline".into());
                Some(format!("core:usb:{generation}"))
            }
            provider => Some(provider.into()),
        },
    }
}

pub(crate) fn resource_entry(path: &str) -> Result<crate::core::files::Entry, String> {
    match owner(path).map_err(String::from)? {
        ResourceOwner::LocalFs => crate::core::files::entry(std::path::Path::new(path)),
        ResourceOwner::PluginMount { .. } if crate::remote::is_remote(path) => {
            crate::remote::stat(path)
        }
        ResourceOwner::PluginMount { .. } => crate::plugins::vfs::stat(path),
        ResourceOwner::NativeVfs("core:usb-fs") => {
            let route = crate::usbfs::route(path).ok_or("invalid USB resource")?;
            crate::usbfs::entry(&route)
        }
        ResourceOwner::NativeVfs(_) => native_vfs::resource_entry(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_virtual_directories_are_native_storage_providers() {
        for path in ["recent://", "applications://"] {
            assert!(matches!(resolve(path).unwrap(), Route::NativeProvider), "{path}");
        }
    }

    #[test]
    fn unknown_and_revoked_uri_schemes_cannot_become_local_fs() {
        for path in [
            "unknown://not-a-file",
            "unknown:opaque-resource",
            "fiex:unregistered",
            "fiex://mount/nonexistent/resource",
            "",
        ] {
            assert!(matches!(resolve(path), Ok(Route::Plugin) | Err(_)), "{path}");
        }
    }

    #[test]
    fn uri_detection_preserves_windows_drive_paths() {
        assert!(!unregistered_scheme(r"C:\\Users\\Example"));
        assert!(!unregistered_scheme("D:/Documents"));
        assert!(unregistered_scheme("unknown:opaque"));
        assert!(unregistered_scheme("web+custom:resource"));
    }

    #[test]
    fn real_host_paths_use_the_registered_native_provider() {
        let path = std::env::temp_dir().join("fiex-provider-registry-local");
        assert!(matches!(resolve(&path.to_string_lossy()), Ok(Route::NativeProvider)));
    }
}
