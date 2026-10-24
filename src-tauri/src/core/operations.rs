//! `lib.rs` pins this module to `core/operations.rs`, so this file only
//! forwards to `core/operations/`; the implementation is split by concern
//! there.
#[path = "operations/mod.rs"]
mod imp;

pub use imp::*;
