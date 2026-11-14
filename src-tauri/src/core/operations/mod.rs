//! Long-running file operations shared by the desktop commands: cancellation,
//! the undo queue, and the create/rename/copy/delete primitives.
//!
//! The implementation is split by concern:
//! - `queue`: `Operations` with the undo stack and transfer bookkeeping,
//! - `fs_ops`: create, rename, and recycle primitives; transfer staging lives in `core::transfers`.

mod fs_ops;
mod queue;

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub use fs_ops::recycle;
pub use queue::Operations;

/// Shared cancel switch for long operations in one app instance.
/// `begin` resets it; `cancel` flips it; the transfer pipeline checks it at safe points.
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

pub use crate::core::operation_types::{CANCELLED, OperationResult};

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
