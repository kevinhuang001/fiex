//! Remote listing, download (provider read streams) and device ejection.

use super::{
    config::{
        connection_for_path, ensure_provider_access, local_for, provider_config, rel_string,
        safe_child_name,
    },
    store::{load_stored, StoredConnection},
    sync::{load_meta, save_meta},
};
use crate::plugins::provider_registry::FsOperation;
use std::{
    fs,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

mod listing;
pub use listing::*;

mod streams;
pub use streams::*;

mod downloads;
pub use downloads::*;

mod cache;
pub use cache::*;

#[cfg(test)]
mod tests;

/// Plugins never receive absolute host paths, even for downloads.
/// Core opens a bounded stream, pulls offset chunks over IPC, and only Core
/// creates or commits the destination file. A failed or cancelled transfer
/// drops its staging file. No provider can write into the host filesystem.
pub type ProgressFn = dyn Fn(u64, Option<u64>) + Send + Sync;

#[derive(Clone, Default)]
pub struct TransferOptions {
    pub cancel: Option<Arc<AtomicBool>>,
    pub progress: Option<Arc<ProgressFn>>,
}

impl TransferOptions {
    pub(crate) fn cancelled(&self) -> bool {
        self.cancel
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
    }
    pub(crate) fn report(&self, done: u64, total: Option<u64>) {
        if let Some(progress) = &self.progress {
            progress(done, total);
        }
    }
}
