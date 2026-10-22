pub mod archives;
pub mod checksums;
#[cfg(feature = "ci-driver")]
mod ci_driver;
#[cfg(feature = "desktop")]
mod desktop;
pub mod drag_session;
pub mod files;
pub mod git;
pub mod git_history;
pub mod git_remote;
pub mod ignores;
#[cfg(all(feature = "desktop", target_os = "macos"))]
mod macos_material;
#[cfg(all(feature = "desktop", target_os = "linux"))]
mod native_drag;
pub mod operations;
pub mod plugin_host;
#[path = "../../sdk/rust/src/lib.rs"]
pub mod plugin_sdk;
pub mod recent;
pub mod remote;
pub mod search;
pub mod tags;
pub mod terminals;
pub mod transfers;
#[cfg(feature = "usbfs")]
pub mod usbfs;
#[cfg(feature = "usbfs")]
pub mod virtual_transfers;
pub mod volumes;
#[cfg(feature = "desktop")]
pub use desktop::run;
