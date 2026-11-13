//! Provider-specific callbacks attached during native VFS registration.

mod applications;
mod recent;
mod shared;
mod storage;
mod trash;

pub(super) use applications::ApplicationsOperations;
pub(super) use recent::RecentOperations;
pub(super) use storage::{LocalStorageOperations, UsbStorageOperations};
pub(super) use trash::TrashOperations;
