//! `lib.rs` pins this module to `core/transfers.rs`, so this file only
//! forwards to `core/transfers/`; the implementation is split by concern
//! there.
#[path = "transfers/mod.rs"]
mod imp;

pub use imp::*;
