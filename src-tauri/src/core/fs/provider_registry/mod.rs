//! Core-owned dispatch into the registered storage providers.
//!
//! Operation groups are separate modules; plugin mount adaptation is isolated
//! in `plugin`, and owner/identity classification in `routing`.
use super::{
    FsError, FsErrorKind, ReadHandle, ResourceBroker, WriteHandle,
    handles::{ProviderReadHandle, ProviderWriteHandle},
};

mod access;
mod plugin;
mod reads;
mod routing;
mod search;
mod writes;

pub(super) use access::require;
pub(super) use reads::{desktop_entry, desktop_listing, list, native_export_path, open_read, stat};
pub(super) use search::{search, search_stream};
pub(super) use writes::{
    begin_write, begin_write_sized, mkdir, move_to_trash, preflight, remove, rename, write_stream,
};

pub(crate) use routing::{
    ChildKind, ResourceAccess, ResourceOwner, child_path, identity, is_plugin_mount, owner,
    parent_path, reference_mount_id, resource_access, resource_entry, validate_child,
};
