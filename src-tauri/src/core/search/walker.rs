//! Application adapter around the standalone bounded filesystem walker.
use super::{BatchSink, SearchBatch, SearchMode, RESULT_LIMIT};
use crate::core::{
    files::{entry, Entry},
    ignores::IgnoreSet,
};
use fiex_search::{walk, SearchWalkStats};
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Instant,
};

pub(super) use fiex_search::BINARY_PROBE;

#[allow(clippy::too_many_arguments)]
fn walk_entries(
    root: &Path,
    query: &str,
    mode: SearchMode,
    hidden: bool,
    ignores: &[String],
    limit: usize,
    cancelled: &dyn Fn() -> bool,
    emit: &mut dyn FnMut(Entry) -> Result<(), String>,
) -> Result<SearchWalkStats, String> {
    let filter = Arc::new(IgnoreSet::new(ignores));
    walk(
        root,
        query,
        mode,
        hidden,
        limit,
        cancelled,
        Arc::new(move |path| filter.is_ignored(path)),
        entry,
        emit,
    )
}

/// Adapter-neutral filename search used by the local/virtual storage adapters.
pub(crate) fn search_directory(
    root: &Path,
    query: &str,
    hidden: bool,
    ignores: &[String],
    limit: usize,
    cancelled: &dyn Fn() -> bool,
) -> (Vec<Entry>, bool, usize) {
    let found = Mutex::new(Vec::new());
    let stats = {
        let mut on_batch = |batch: SearchBatch| {
            if let Ok(mut found) = found.lock() {
                found.extend(batch.entries);
            }
        };
        let mut sink = BatchSink::new(&mut on_batch, Instant::now(), limit);
        let stats = walk_entries(
            root,
            query,
            SearchMode::Name,
            hidden,
            ignores,
            limit,
            cancelled,
            &mut |entry| {
                sink.push(entry);
                Ok(())
            },
        )
        .unwrap_or_default();
        sink.truncated = stats.truncated;
        sink.skipped = stats.skipped;
        sink.scanned = stats.scanned;
        sink.incomplete = stats.incomplete;
        sink.flush(false);
        stats
    };
    let entries = found.into_inner().unwrap_or_default();
    (entries, stats.truncated, stats.skipped)
}

/// Stream the shared filesystem walk through the in-process SearchProvider
/// adapter while keeping cancellation and result validation in Core.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_native_search_stream(
    root: &Path,
    query: &str,
    mode: SearchMode,
    hidden: bool,
    ignores: &[String],
    limit: usize,
    cancelled: &dyn Fn() -> bool,
    emit: &mut dyn FnMut(Entry) -> Result<(), String>,
) -> Result<bool, String> {
    let stats = walk_entries(
        root,
        query,
        mode,
        hidden,
        ignores,
        limit.clamp(1, RESULT_LIMIT),
        cancelled,
        emit,
    )?;
    Ok(stats.truncated || stats.incomplete)
}
