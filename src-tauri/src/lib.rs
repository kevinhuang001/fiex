pub mod archives;
#[cfg(feature = "desktop")]
mod desktop;
pub mod files;
pub mod git;
pub mod git_history;
pub mod ignores;
#[cfg(all(feature = "desktop", target_os = "macos"))]
mod macos_material;
#[cfg(all(feature = "desktop", target_os = "linux"))]
mod native_drag;
pub mod operations;
pub mod remote;
pub mod search;
pub mod tags;
pub mod terminals;
#[cfg(feature = "usbfs")]
pub mod usbfs;
pub mod volumes;
#[cfg(feature = "desktop")]
pub use desktop::run;
