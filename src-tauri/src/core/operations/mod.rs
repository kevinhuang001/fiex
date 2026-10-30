//! Long-running file operations shared by the desktop commands: cancellation,
//! the undo queue, and the create/rename/copy/delete primitives.
//!
//! The implementation is split by concern:
//! - `queue`: `Operations` with the undo stack and transfer bookkeeping,
//! - `fs_ops`: filesystem primitives and the copy engine they build on.

mod fs_ops;
mod queue;

use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

pub use fs_ops::recycle;
pub(crate) use fs_ops::{copy_stream, copy_tree_with_progress};
pub use queue::Operations;

/// Error returned by every long operation once the shared flag is set.
pub const CANCELLED: &str = "Cancelled: user cancelled operation";

/// Shared cancel switch for the long-running operations of one app instance.
/// `begin_operation` resets it; `cancel_operation` flips it; the loops below
/// poll it and bail out with `CANCELLED` at a safe point.
#[derive(Clone, Default)]
pub struct OperationCancel(Arc<AtomicBool>);

impl OperationCancel {
    pub fn begin(&self) {
        self.0.store(false, Ordering::SeqCst);
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
    pub fn flag(&self) -> Arc<AtomicBool> {
        self.0.clone()
    }
}

pub(crate) fn check_cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        return Err(CANCELLED.into());
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) enum Undo {
    Rename(PathBuf, PathBuf),
    Created(PathBuf),
    Replaced(PathBuf, tempfile::TempDir),
}

#[derive(Default, Serialize)]
pub struct OperationResult {
    pub paths: Vec<String>,
    pub errors: Vec<String>,
    pub skipped: usize,
    pub conflicts: Vec<crate::core::transfers::Conflict>,
}

pub(crate) fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

pub fn unique(path: &Path) -> PathBuf {
    if !exists(path) {
        return path.into();
    }
    let stem = if path.is_dir() {
        path.file_name()
    } else {
        path.file_stem()
    }
    .unwrap_or_default()
    .to_string_lossy();
    let ext = (!path.is_dir())
        .then(|| path.extension())
        .flatten()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();
    for i in 2.. {
        let next = path.with_file_name(format!("{stem} ({i}){ext}"));
        if !exists(&next) {
            return next;
        }
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::OperationCancel;

    #[test]
    fn operation_cancel_flag_roundtrip() {
        let cancel = OperationCancel::default();
        assert!(!cancel.is_cancelled());
        cancel.cancel();
        assert!(cancel.is_cancelled());
        cancel.begin();
        assert!(!cancel.is_cancelled());
    }
}
