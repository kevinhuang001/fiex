pub mod archives;
#[cfg(feature = "desktop")]
mod desktop;
pub mod files;
#[cfg(all(feature = "desktop", target_os = "macos"))]
mod macos_material;
pub mod operations;
pub mod search;
pub mod terminals;
#[cfg(feature = "desktop")]
pub use desktop::run;
