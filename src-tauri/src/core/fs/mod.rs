//! Core-owned file access. Resource identities are resolved only inside an
//! operation scope; Providers never receive plugin-supplied filesystem paths.
mod binding;
mod broker;
mod cache_identity;
pub(crate) mod changes;
pub(crate) mod decoration_cache;
pub(crate) mod directory_cache;
mod export;
mod list_service;
pub(crate) mod mapped_access;
mod mutation_overlay;
pub(crate) mod path_utils;
mod preflight;
pub(crate) mod presentation;
mod transfer;
#[cfg(any(feature = "desktop", test))]
pub(crate) use binding::create_file_with_bytes;
pub(crate) use binding::trusted_snapshot;
pub(crate) use binding::{child, parent_path, trusted, trusted_with_user_action};
#[cfg(feature = "desktop")]
pub(crate) use binding::{create, move_to_trash, rename_path};
pub(crate) use export::export as export_for_external_drag;
#[cfg(feature = "desktop")]
pub(crate) use export::initialize as initialize_exports;
pub use fiex_fs::{CHUNK, FsError, FsErrorKind, ReadHandle, WriteHandle};
pub use transfer::transfer;
pub(crate) mod provider_registry;
pub use broker::ResourceBroker;
mod streams;
use crate::plugins::resource::fs_access::Scope;
use fiex_provider_contracts::FsOperation;
use serde_json::Value;
pub use streams::{cleanup, http_body, stream_call, stream_permission};

mod handles;

pub(crate) fn clear_preflight_availability_cache() {
    preflight::clear_availability_cache();
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod provider_tests;
