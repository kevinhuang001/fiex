//! Per-file progress samples for the copy and move engine.

use std::{
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

/// Emits `(done, total, current file)` samples: once at the start, once per
/// copied file and once at the end when the plan knows its total up front.
pub(super) struct Progress<'a> {
    done: AtomicU64,
    total: u64,
    emit: &'a (dyn Fn(u64, u64, Option<&str>) + Sync),
}

impl<'a> Progress<'a> {
    pub(super) fn new(total: u64, emit: &'a (dyn Fn(u64, u64, Option<&str>) + Sync)) -> Self {
        let progress = Self {
            done: AtomicU64::new(0),
            total,
            emit,
        };
        if total > 0 {
            (progress.emit)(0, total, None);
        }
        progress
    }

    /// Count one copied file and report the new total.
    pub(super) fn record(&self, path: &Path) {
        let done = self.done.fetch_add(1, Ordering::Relaxed) + 1;
        (self.emit)(
            done,
            self.total,
            path.file_name().and_then(|name| name.to_str()),
        );
    }

    /// Moves finish at file-system speed, so their files are reported in one
    /// step once the whole task succeeded.
    pub(super) fn record_files(&self, path: &Path, files: u64) {
        for _ in 0..files {
            self.record(path);
        }
    }

    pub(super) fn finish(&self) {
        if self.total > 0 {
            (self.emit)(self.total, self.total, None);
        }
    }
}
