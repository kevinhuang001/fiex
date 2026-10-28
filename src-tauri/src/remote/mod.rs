//! Remote connections: the connection registry, provider configuration,
//! provider-backed file access, and the local download cache.

mod cache;
mod config;
mod credentials;
mod files;
#[cfg(feature = "desktop")]
mod mutate;
mod reconcile;
mod store;
mod sync;

use std::sync::atomic::{AtomicU64, Ordering};

/// Background reconciliation cadence in seconds. Settings push this from the
/// UI; the scheduler re-reads it while it waits so changes apply immediately.
static SYNC_INTERVAL_SECS: AtomicU64 = AtomicU64::new(30);

pub fn set_sync_interval(seconds: u64) {
    SYNC_INTERVAL_SECS.store(seconds.clamp(5, 24 * 60 * 60), Ordering::Relaxed);
}

#[cfg(feature = "desktop")]
pub(crate) fn sync_interval() -> u64 {
    SYNC_INTERVAL_SECS.load(Ordering::Relaxed)
}

/// Sleep until the next round is due, re-reading the interval once a second so
/// a settings change applies without waiting out the previous sleep.
#[cfg(feature = "desktop")]
pub(crate) fn wait_for_round() {
    let started = std::time::Instant::now();
    while started.elapsed().as_secs() < sync_interval() {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

pub use cache::CacheChange;
#[cfg(feature = "desktop")]
pub(crate) use config::provider_config;
pub(crate) use config::{connection_by_mount, ensure_provider_access};
pub use config::{connection_for_path, is_remote};
pub(crate) use files::adapter_node;
pub use files::{
    cached_path, download, download_dir, eject, ensure_downloaded, list_listing, provider_list,
    stat, ProgressFn, TransferOptions,
};
#[cfg(feature = "desktop")]
pub(crate) use mutate::{create, create_file, remove, rename};
pub use reconcile::{reconcile, reconcile_connection, resolve};
pub use store::{
    connection_config, list_connections, open_navigation, remove_connection, save_connection,
    update_connection_session, ConnectionInfo, ConnectionInput, StoredConnection,
};
pub use sync::{sync, sync_one, uncache, unpin};
