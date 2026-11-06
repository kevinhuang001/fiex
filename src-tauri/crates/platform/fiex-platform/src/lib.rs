//! Operating-system services that can run without the desktop application.

mod file_trash;
mod open_with;
pub mod recycle_bin;
pub mod system_proxy;

pub use file_trash::move_to_trash;
#[cfg(windows)]
pub use open_with::{enumerate_windows_applications, WindowsApplication};
pub use open_with::{
    open_with, open_with_application, open_with_applications, open_with_candidates,
    OpenWithCandidate,
};
