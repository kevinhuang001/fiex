#[cfg(feature = "ci-driver")]
mod ci_driver;
#[cfg(feature = "desktop")]
mod desktop;
#[path = "core/drag_session.rs"]
pub mod drag_session;
#[path = "core/files.rs"]
pub mod files;
#[path = "core/ignores.rs"]
pub mod ignores;
#[cfg(all(feature = "desktop", target_os = "macos"))]
mod macos_material;
#[cfg(all(feature = "desktop", target_os = "linux"))]
mod native_drag;
#[path = "core/operations.rs"]
pub mod operations;
#[path = "plugins/decorations.rs"]
pub mod plugin_decorations;
#[path = "plugins/fs_access.rs"]
pub mod plugin_fs_access;
#[path = "plugins/host.rs"]
pub mod plugin_host;
#[path = "plugins/installer.rs"]
pub mod plugin_installer;
#[path = "plugins/ipc.rs"]
pub mod plugin_ipc;
#[path = "plugins/permissions.rs"]
pub mod plugin_permissions;
#[path = "plugins/signing.rs"]
pub mod plugin_signing;
pub use fiex_plugin_sdk as plugin_sdk;
#[path = "plugins/sandbox.rs"]
pub mod plugin_sandbox;
#[path = "plugins/search.rs"]
pub mod plugin_search;
#[path = "plugins/storage.rs"]
pub mod plugin_storage;
#[path = "plugins/vfs.rs"]
pub mod plugin_vfs;
#[path = "core/recent.rs"]
pub mod recent;
pub mod remote;
#[path = "core/search.rs"]
pub mod search;
pub mod system_proxy;
#[path = "core/tags.rs"]
pub mod tags;
#[path = "core/terminals.rs"]
pub mod terminals;
#[path = "core/transfers.rs"]
pub mod transfers;
#[cfg(feature = "usbfs")]
pub mod usbfs;
#[cfg(feature = "usbfs")]
pub mod virtual_transfers;
#[path = "core/volumes.rs"]
pub mod volumes;
#[cfg(feature = "desktop")]
pub use desktop::run;
