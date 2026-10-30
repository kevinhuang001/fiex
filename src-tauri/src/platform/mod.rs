//! OS integration shared by desktop adapters and plugin services.
#[cfg(all(feature = "desktop", target_os = "macos"))]
pub(crate) mod macos_material;
#[cfg(all(feature = "desktop", target_os = "linux"))]
pub(crate) mod native_drag;
pub mod system_proxy;
#[cfg(target_os = "windows")]
pub(crate) mod windows;
