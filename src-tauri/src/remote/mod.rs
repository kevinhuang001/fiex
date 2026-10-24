//! Remote connections: the connection registry, provider configuration,
//! provider-backed file access, and the local download cache.

mod config;
mod files;
mod reconcile;
mod store;
mod sync;

pub(crate) use config::{connection_by_mount, ensure_provider_access};
pub use config::{connection_for_path, is_remote};
pub use files::{
    cached_path, download, download_dir, eject, ensure_downloaded, list_listing, provider_list,
    stat, ProgressFn, TransferOptions,
};
pub use reconcile::{reconcile, reconcile_connection, resolve};
pub use store::{
    list_connections, remove_connection, save_connection, ConnectionInfo, ConnectionInput,
    StoredConnection,
};
pub use sync::{sync, sync_one, unsync};
