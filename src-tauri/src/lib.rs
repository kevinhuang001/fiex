pub mod archives;
#[cfg(feature = "ci-driver")]
mod ci_driver;
#[cfg(feature = "desktop")]
mod desktop;
#[cfg(all(feature = "desktop", target_os = "macos"))]
mod macos_material;
pub mod drag_session;
pub mod files;
pub mod git;
pub mod git_history;
pub mod git_remote;
pub mod ignores;
pub mod recent;
#[cfg(all(feature = "desktop", target_os = "linux"))]
mod native_drag;
pub mod operations;
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
