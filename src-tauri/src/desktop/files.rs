//! File system commands: directory listing, entry metadata and previews,
//! creation, renaming, transfers, permanent deletion and trash handling,
//! directory watching, and search.

use crate::{
    core::files,
    core::operations::{self, OperationCancel, OperationResult, Operations},
    core::recent,
    core::search::{SearchBatch, SearchState},
    core::terminals,
    core::transfers::Resolution,
    core::volumes,
    remote,
};
use notify::Watcher;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Mutex};
use tauri::{ipc::Channel, Emitter, Manager};

#[macro_use]
mod listing;
pub(crate) use listing::*;

#[macro_use]
mod creation;
pub(crate) use creation::*;

#[macro_use]
mod transfer;
pub(crate) use transfer::*;

#[macro_use]
mod recycle;
pub(crate) use recycle::*;

#[macro_use]
mod search;
pub(crate) use search::*;

#[macro_use]
mod watch;
pub(crate) use watch::*;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OperationProgress {
    done: u64,
    total: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    step: Option<String>,
    /// `bytes` renders sizes in the activity centre instead of file counts.
    #[serde(skip_serializing_if = "Option::is_none")]
    unit: Option<String>,
}

pub(crate) fn progress_emitter(
    channel: Channel<OperationProgress>,
) -> impl Fn(u64, u64, Option<&str>) + Send + Sync + 'static {
    move |done, total, step| {
        let _ = channel.send(OperationProgress {
            done,
            total,
            step: step.map(str::to_string),
            unit: None,
        });
    }
}

pub(crate) type ByteProgress = std::sync::Arc<dyn Fn(u64, Option<u64>, Option<&str>) + Send + Sync>;

/// Download progress is byte-based; the activity centre formats it as sizes.
pub(crate) fn byte_progress_emitter(channel: Channel<OperationProgress>) -> ByteProgress {
    std::sync::Arc::new(move |done, total, step| {
        let _ = channel.send(OperationProgress {
            done,
            total: total.unwrap_or(0),
            step: step.map(str::to_string),
            unit: Some("bytes".into()),
        });
    })
}

pub(crate) type Watch = Mutex<HashMap<String, (String, notify::RecommendedWatcher)>>;
