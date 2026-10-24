//! `lib.rs` pins this module to `core/volumes.rs`, so this file only forwards
//! to `core/volumes/`; the implementation is split by concern there.
#[path = "volumes/mod.rs"]
mod imp;

pub use imp::*;
