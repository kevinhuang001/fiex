//! Shared operation contracts used by Core filesystem and transfer services.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Error returned by long-running operations after the shared cancel flag is set.
pub const CANCELLED: &str = "Cancelled: user cancelled operation";

/// Check cancellation at a safe point shared by filesystem use cases.
pub(crate) fn check_cancelled(cancel: &std::sync::atomic::AtomicBool) -> Result<(), String> {
    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
        Err(CANCELLED.into())
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileVersion {
    pub path: String,
    pub size: u64,
    pub modified: u64,
    pub is_dir: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Conflict {
    pub key: String,
    pub source: FileVersion,
    pub destination: FileVersion,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    Replace,
    Skip,
    KeepBoth,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Resolution {
    pub key: String,
    pub action: Action,
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
    pub conflicts: Vec<Conflict>,
}
