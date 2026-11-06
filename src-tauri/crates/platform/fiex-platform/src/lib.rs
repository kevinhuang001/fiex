//! Operating-system services that can run without the desktop application.

mod file_trash;
pub mod recycle_bin;
pub mod system_proxy;

pub use file_trash::move_to_trash;
