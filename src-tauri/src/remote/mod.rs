//! Remote connections: the connection registry, provider configuration,
//! provider-backed file access, and the local download cache.

mod config;
mod files;
mod store;
mod sync;

pub use config::{connection_for_path, is_remote};
pub use files::{
    download, download_dir, eject, ensure_downloaded, list_listing, provider_list, ProgressFn,
    TransferOptions,
};
pub use store::{
    list_connections, remove_connection, save_connection, ConnectionInfo, ConnectionInput,
    StoredConnection,
};
pub use sync::{sync, sync_one, unsync};
