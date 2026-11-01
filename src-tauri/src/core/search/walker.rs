//! Bounded local filesystem walker shared by native and builtin search providers.
use super::{BatchSink, SearchBatch, SearchMode, CONTENT_FILE_LIMIT, RESULT_LIMIT};
use crate::core::{files::{entry, Entry}, ignores::IgnoreSet};
use ignore::WalkBuilder;
use std::{
    path::Path,
    sync::{atomic::Ordering, Arc, Mutex},
    time::Instant,
};

const CONTENT_BYTE_BUDGET: u64 = 64 * 1024 * 1024;
const CONTENT_FILE_BUDGET: usize = 5000;
const BINARY_PROBE: usize = 8192;

fn walk_root(
    root: &Path,
    query: &str,
    mode: SearchMode,
    hidden: bool,
    ignores: &[String],
    cancelled: &dyn Fn() -> bool,
    sink: &mut BatchSink<'_>,
) {
    let lowered = query.to_lowercase();
    if !root.is_dir() || lowered.is_empty() {
        return;
    }
    let root = root.to_path_buf();
    let filter_root = root.clone();
    let filter_ignore = Arc::new(IgnoreSet::new(ignores));
    let walker = WalkBuilder::new(&root)
        .hidden(!hidden)
        .ignore(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .parents(false)
        .follow_links(false)
        .same_file_system(true)
        .threads(1)
        .filter_entry(move |item| {
            if item.depth() == 0 {
                return true;
            }
            !inside_bundle(item.path())
                && !filter_ignore.is_ignored(
                    item.path()
                        .strip_prefix(&filter_root)
                        .unwrap_or(item.path()),
                )
        })
        .build();
    let mut content = ContentBudget::default();
    for item in walker {
        if cancelled() {
            return;
        }
        let Ok(item) = item else {
            sink.skipped += 1;
            continue;
        };
        if item.depth() == 0 {
            continue;
        }
        sink.scanned += 1;
        let path = item.path();
        match mode {
            SearchMode::Name => {
                let name = item.file_name().to_string_lossy().to_lowercase();
                if !name.contains(&lowered) {
                    continue;
                }
                match entry(path) {
                    Ok(value) => sink.push(value),
                    Err(_) => sink.skipped += 1,
                }
            }
            SearchMode::Content => {
                if path.is_dir() {
                    continue;
                }
                match content.match_file(path, &lowered) {
                    ContentMatch::Hit => match entry(path) {
                        Ok(value) => sink.push(value),
                        Err(_) => sink.skipped += 1,
                    },
                    ContentMatch::Miss => {}
                    ContentMatch::Skipped => sink.skipped += 1,
                    ContentMatch::Exhausted => {
                        sink.incomplete = true;
                        return;
                    }
                }
            }
        }
        if sink.truncated {
            return;
        }
    }
}

#[derive(Default)]
struct ContentBudget {
    files: usize,
    bytes: u64,
}

enum ContentMatch {
    Hit,
    Miss,
    Skipped,
    Exhausted,
}

impl ContentBudget {
    fn match_file(&mut self, path: &Path, lowered: &str) -> ContentMatch {
        if self.files >= CONTENT_FILE_BUDGET || self.bytes >= CONTENT_BYTE_BUDGET {
            return ContentMatch::Exhausted;
        }
        let Ok(metadata) = std::fs::metadata(path) else {
            return ContentMatch::Skipped;
        };
        if metadata.len() > CONTENT_FILE_LIMIT {
            return ContentMatch::Skipped;
        }
        let Ok(bytes) = std::fs::read(path) else {
            return ContentMatch::Skipped;
        };
        self.files += 1;
        self.bytes += bytes.len() as u64;
        let probe = &bytes[..bytes.len().min(BINARY_PROBE)];
        if probe.contains(&0) {
            return ContentMatch::Skipped;
        }
        let text = String::from_utf8_lossy(&bytes);
        if text.to_lowercase().contains(lowered) {
            ContentMatch::Hit
        } else {
            ContentMatch::Miss
        }
    }
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
    let (truncated, skipped);
    {
        let mut emit = |batch: SearchBatch| {
            if let Ok(mut found) = found.lock() {
                found.extend(batch.entries);
            }
        };
        let mut sink = BatchSink::new(&mut emit, Instant::now(), limit);
        walk_root(
            root,
            query,
            SearchMode::Name,
            hidden,
            ignores,
            cancelled,
            &mut sink,
        );
        sink.flush(false);
        truncated = sink.truncated;
        skipped = sink.skipped;
    }
    (found.into_inner().unwrap_or_default(), truncated, skipped)
}

/// Stream the Core-native search walker through the shared SearchProvider
/// contract while preserving cancellation, result limits and incremental UI
/// delivery. Provider implementations never receive the host path over IPC.
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
    let failed = std::sync::atomic::AtomicBool::new(false);
    let failure = Mutex::new(None::<String>);
    let should_cancel = || cancelled() || failed.load(std::sync::atomic::Ordering::Relaxed);
    let mut emit_batches = |batch: SearchBatch| {
        for entry in batch.entries {
            if should_cancel() {
                break;
            }
            if let Err(error) = emit(entry) {
                if let Ok(mut failure) = failure.lock() {
                    *failure = Some(error);
                }
                failed.store(true, std::sync::atomic::Ordering::Relaxed);
                break;
            }
        }
    };
    let mut sink = BatchSink::new(
        &mut emit_batches,
        Instant::now(),
        limit.clamp(1, RESULT_LIMIT),
    );
    sink.engine = Some("builtin".into());
    walk_root(
        root,
        query,
        mode,
        hidden,
        ignores,
        &should_cancel,
        &mut sink,
    );
    sink.flush(false);
    let truncated = sink.truncated || sink.incomplete;
    drop(sink);
    drop(emit_batches);
    if let Some(error) = failure
        .into_inner()
        .unwrap_or_else(|error| error.into_inner())
    {
        return Err(error);
    }
    Ok(truncated)
}

const BUNDLE_EXTENSIONS: [&str; 5] = ["app", "bundle", "framework", "plugin", "xpc"];

fn is_bundle(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            BUNDLE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
        })
}

fn inside_bundle(path: &Path) -> bool {
    path.ancestors().skip(1).any(is_bundle)
}

