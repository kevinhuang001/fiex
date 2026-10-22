#[cfg(feature = "ci-driver")]
mod ci_driver;
#[cfg(feature = "desktop")]
mod desktop;
#[path = "core/drag_session.rs"]
pub mod drag_session;
#[path = "core/files.rs"]
pub mod files;
pub mod git;
pub mod git_history;
pub mod git_remote;
#[path = "core/ignores.rs"]
pub mod ignores;
#[cfg(all(feature = "desktop", target_os = "macos"))]
mod macos_material;
#[cfg(all(feature = "desktop", target_os = "linux"))]
mod native_drag;
#[path = "core/operations.rs"]
pub mod operations;
#[path = "plugins/installer.rs"]
pub mod plugin_installer;
#[path = "plugins/host.rs"]
pub mod plugin_host;
#[path = "plugins/permissions.rs"]
pub mod plugin_permissions;
#[path = "../../sdk/rust/src/lib.rs"]
pub mod plugin_sdk;
#[path = "plugins/storage.rs"]
pub mod plugin_storage;
#[path = "core/recent.rs"]
pub mod recent;
pub mod remote;
#[path = "core/search.rs"]
pub mod search;
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
