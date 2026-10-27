//! Fiex native library. Module ownership follows functional domains.
#[cfg(feature = "ci-driver")]
mod ci_driver;
pub mod core;
#[cfg(feature = "desktop")]
pub(crate) mod desktop;
pub mod i18n;
pub mod platform;
pub mod plugins;
pub mod remote;
#[cfg(feature = "usbfs")]
pub mod usbfs;
pub use fiex_plugin_sdk as plugin_sdk;

#[cfg(feature = "desktop")]
pub use desktop::run;
