//! Core-owned storage provider selection. Every backend implements the same
//! FsAdapter trait and therefore flows through ResourceBroker's common
//! capability, operation-scope, stream, transfer and error contracts.
//!
//! A URI that is not registered is never promoted to a local path. Selection
//! cannot be influenced by extension metadata or plugin response payloads.
use super::{builtin, local, plugin, usb, FsAdapter, FsError, ResourceBroker};
use crate::{core::volumes::virtual_resolvers, plugins::mounts, usbfs};

enum Route {
    Plugin,
    Usb(usbfs::Route),
    NativeView,
    Local,
}

fn resolve(path: &str) -> Result<Route, FsError> {
    // The mount service claims revoked and invalid mount URIs too, so these
    // must fail in the plugin resolver instead of falling through to Local.
    if mounts::handles(path) {
        return Ok(Route::Plugin);
    }
    if let Some(route) = usbfs::route(path) {
        return Ok(Route::Usb(route));
    }
    if virtual_resolvers::view_for(path).is_some() {
        return Ok(Route::NativeView);
    }
    if path.is_empty() || path.contains("://") {
        return Err("Unsupported: unregistered filesystem URI".into());
    }
    Ok(Route::Local)
}

pub(super) fn adapter(resource: &ResourceBroker) -> Result<Box<dyn FsAdapter + '_>, FsError> {
    match resolve(&resource.path)? {
        Route::Plugin => Ok(Box::new(plugin::PluginFsAdapter::new(resource)?)),
        Route::Usb(route) => Ok(Box::new(usb::UsbFsAdapter::new(route))),
        Route::NativeView => Ok(Box::new(builtin::BuiltinFsAdapter(resource))),
        Route::Local => {
            resource.scope.authorize(&resource.path)?;
            Ok(Box::new(local::LocalFsAdapter(resource)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_virtual_directories_are_native_storage_providers() {
        for path in ["recent://", "applications://"] {
            assert!(
                matches!(resolve(path).unwrap(), Route::NativeView),
                "{path}"
            );
        }
    }

    #[test]
    fn unknown_and_revoked_uri_schemes_cannot_become_local_fs() {
        for path in [
            "unknown://not-a-file",
            "fiex://mount/nonexistent/resource",
            "",
        ] {
            assert!(
                matches!(resolve(path), Ok(Route::Plugin) | Err(_)),
                "{path}"
            );
            assert!(!matches!(resolve(path), Ok(Route::Local)), "{path}");
        }
    }

    #[test]
    fn real_host_paths_use_the_native_adapter() {
        let path = std::env::temp_dir().join("fiex-provider-registry-local");
        assert!(matches!(resolve(&path.to_string_lossy()), Ok(Route::Local)));
    }
}
