//! Local copy and move pipeline: conflict detection, planning, execution and
//! progress reporting.
//!
//! The implementation is split by concern:
//! - `conflicts`: file versions and the decisions made about collisions,
//! - `copy`: the walk/plan and the copy or move engine itself,
//! - `progress`: per-file progress samples.

mod conflicts;
mod copy;
mod progress;

use std::sync::atomic::AtomicBool;

pub use conflicts::{Action, Conflict, FileVersion, Resolution};
pub(crate) use copy::{execute, move_tree};

/// Undo moves cannot be interrupted; this flag is never set.
pub(crate) static NEVER_CANCELLED: AtomicBool = AtomicBool::new(false);
