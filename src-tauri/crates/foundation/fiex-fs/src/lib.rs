//! Shared filesystem service contracts used by the application adapters.

mod error;
mod stream;

pub use error::{FsError, FsErrorKind};
pub use stream::{ReadHandle, WriteHandle, CHUNK};
