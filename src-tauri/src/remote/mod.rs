//! Remote connections: the connection registry, provider configuration,
//! provider-backed file access, and the local download cache.

mod config;
mod files;
mod reconcile;
mod store;
mod sync;

#[cfg(feature = "desktop")]
pub(crate) use config::provider_config;
pub(crate) use config::{connection_by_mount, ensure_provider_access};
pub use config::{connection_for_path, is_remote};
pub use files::{
    cached_path, download, download_dir, eject, ensure_downloaded, list_listing, provider_list,
    stat, ProgressFn, TransferOptions,
};
pub use reconcile::{reconcile, reconcile_connection, resolve};
#[cfg(feature = "desktop")]
pub(crate) use store::load_stored;
pub use store::{
    list_connections, open_navigation, remove_connection, save_connection,
    update_connection_session, ConnectionInfo, ConnectionInput, StoredConnection,
};
pub use sync::{sync, sync_one, unsync};
