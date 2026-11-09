//! Shared filesystem service contracts used by the application adapters.

mod error;
mod provider_stream;
mod stream;

pub use error::{FsError, FsErrorKind};
pub use provider_stream::{ProviderReadHandle, ProviderWriteHandle};
pub use stream::{CHUNK, ReadHandle, WriteHandle};
