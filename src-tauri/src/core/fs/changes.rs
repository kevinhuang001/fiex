//! Successful mutation boundary. Failure and aborted writes never invalidate data.
use std::sync::atomic::{AtomicU64, Ordering};
static REVISION: AtomicU64 = AtomicU64::new(0);
pub(crate) fn revision() -> u64 {
    REVISION.load(Ordering::Acquire)
}
pub(crate) fn committed(path: &str) {
    REVISION.fetch_add(1, Ordering::AcqRel);
    if super::directory_cache::enabled(path) {
        super::directory_cache::drop_subtree(path);
        if let Ok(parent) = super::binding::parent_path(path) {
            super::directory_cache::invalidate_directory(&parent);
        }
    }
}
