//! Live search with independent filename/content modes.
//!
//! Local FS queries can be served by an installed `SearchProvider` (for example
//! Everything) or by the builtin walker, which is also the last Auto candidate.
//! Only one engine runs per query; Core validates every hit, deduplicates it and
//! streams partial batches to the UI before the walk finishes.
use crate::core::files::Entry;
mod adapter_content;
mod providers;
mod walker;
pub use providers::accessible_roots;
use providers::search_root;
#[cfg(test)]
use providers::virtual_search_root;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
pub(crate) use walker::{run_native_search_stream, search_directory};

pub const RESULT_LIMIT: usize = 500;
const BATCH_SIZE: usize = 50;
const BATCH_INTERVAL: Duration = Duration::from_millis(120);
/// Content readers stay bounded and skip format extraction, avoiding a disk
/// walk that reads every byte of every supported document on each keystroke.
pub(crate) use fiex_search::CONTENT_FILE_LIMIT;

/// Search mode is shared with SDK VFS and provider DTOs.
pub use fiex_provider_contracts::SearchMode;

/// Which engine the user asked for. `Auto` picks the first available provider
/// that declares the mode and falls back to the builtin walker last.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SearchEngine {
    #[default]
    Auto,
    Builtin,
    Provider(String),
}

impl<'de> Deserialize<'de> for SearchEngine {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "" | "auto" => Self::Auto,
            "builtin" => Self::Builtin,
            other => Self::Provider(other.trim_start_matches("plugin:").to_string()),
        })
    }
}

#[derive(Clone, Debug)]
pub struct SearchRequest {
    pub root: String,
    pub query: String,
    pub mode: SearchMode,
    pub engine: SearchEngine,
    pub hidden: bool,
    pub ignores: Vec<String>,
    pub limit: usize,
}

impl SearchRequest {
    pub fn new(root: &str, query: &str) -> Self {
        Self {
            root: root.into(),
            query: query.into(),
            mode: SearchMode::Name,
            engine: SearchEngine::Auto,
            hidden: false,
            ignores: Vec::new(),
            limit: RESULT_LIMIT,
        }
    }
}

#[derive(Clone, Default)]
pub struct SearchState {
    generations: Arc<Mutex<HashMap<String, Arc<AtomicU64>>>>,
}

/// A single search invocation. Cancelling or superseding the scope advances
/// the generation and invalidates every cloned ticket.
#[derive(Clone)]
pub struct SearchTicket {
    generation: Arc<AtomicU64>,
    ticket: u64,
}

impl SearchTicket {
    pub fn cancelled(&self) -> bool {
        self.generation.load(Ordering::Relaxed) != self.ticket
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchBatch {
    pub entries: Vec<Entry>,
    pub done: bool,
    pub elapsed_ms: u128,
    pub truncated: bool,
    pub skipped: usize,
    /// Entries visited by the walker (files and directories).
    pub scanned: usize,
    /// The engine hit a budget and did not examine the whole subtree.
    pub incomplete: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
    /// Engine that produced this batch (`builtin` or a provider name).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    /// Virtual-folder results are complete snapshots: replace the visible list
    /// instead of appending. Local walker batches always keep this false.
    #[serde(default)]
    pub replace: bool,
}

/// Streams bounded batches. Intermediate batches never claim completion; the
/// final batch carries the accumulated counters exactly once.
struct BatchSink<'a> {
    emit: &'a mut dyn FnMut(SearchBatch),
    started: Instant,
    last: Instant,
    buffer: Vec<Entry>,
    total: usize,
    limit: usize,
    truncated: bool,
    skipped: usize,
    scanned: usize,
    incomplete: bool,
    warning: Option<String>,
    engine: Option<String>,
    done_sent: bool,
}

impl<'a> BatchSink<'a> {
    fn new(emit: &'a mut dyn FnMut(SearchBatch), started: Instant, limit: usize) -> Self {
        Self {
            emit,
            started,
            last: Instant::now(),
            buffer: Vec::new(),
            total: 0,
            limit,
            truncated: false,
            skipped: 0,
            scanned: 0,
            incomplete: false,
            warning: None,
            engine: None,
            done_sent: false,
        }
    }

    fn push(&mut self, entry: Entry) {
        if self.total >= self.limit {
            self.truncated = true;
            return;
        }
        self.total += 1;
        self.buffer.push(entry);
        if self.buffer.len() >= BATCH_SIZE || self.last.elapsed() >= BATCH_INTERVAL {
            self.flush(false);
        }
    }

    fn flush(&mut self, done: bool) {
        if self.buffer.is_empty() && !done {
            return;
        }
        (self.emit)(SearchBatch {
            entries: std::mem::take(&mut self.buffer),
            done,
            elapsed_ms: self.started.elapsed().as_millis(),
            truncated: self.truncated,
            skipped: self.skipped,
            scanned: self.scanned,
            incomplete: self.incomplete,
            warning: self.warning.clone(),
            engine: self.engine.clone(),
            replace: false,
        });
        if done {
            self.done_sent = true;
        }
        self.last = Instant::now();
    }

    /// Virtual-folder batches carry a complete result set. They bypass the
    /// incremental buffer: the first batch replaces the list, the terminal
    /// batch either confirms it (empty, no replace) or replaces it again.
    fn emit_snapshot(
        &mut self,
        entries: Vec<Entry>,
        done: bool,
        replace: bool,
        truncated: bool,
        skipped: usize,
        scanned: usize,
    ) {
        (self.emit)(SearchBatch {
            entries,
            done,
            elapsed_ms: self.started.elapsed().as_millis(),
            truncated,
            skipped,
            scanned,
            incomplete: self.incomplete,
            warning: self.warning.clone(),
            engine: self.engine.clone(),
            replace,
        });
        if done {
            self.done_sent = true;
        }
        self.last = Instant::now();
    }

    fn finish(&mut self) {
        if !self.done_sent {
            self.flush(true);
        }
    }
}

impl SearchState {
    fn generation(&self, scope: &str) -> Arc<AtomicU64> {
        self.generations
            .lock()
            .unwrap()
            .entry(scope.to_owned())
            .or_default()
            .clone()
    }

    pub fn cancel(&self, scope: &str) {
        self.generation(scope).fetch_add(1, Ordering::SeqCst);
    }

    pub fn begin(&self, scope: &str) -> SearchTicket {
        let generation = self.generation(scope);
        let ticket = generation.fetch_add(1, Ordering::SeqCst) + 1;
        SearchTicket { generation, ticket }
    }

    /// Run one search session: a new generation cancels the previous one and
    /// every late batch is rejected by the caller via its session token.
    pub fn stream(
        &self,
        scope: &str,
        request: &SearchRequest,
        emit: impl Fn(SearchBatch) + Send + Sync,
    ) -> Result<(), String> {
        let started = Instant::now();
        let generation = self.generation(scope);
        let ticket = generation.fetch_add(1, Ordering::SeqCst) + 1;
        let cancelled = || generation.load(Ordering::Relaxed) != ticket;
        let limit = request.limit.clamp(1, RESULT_LIMIT);
        let mut emit = emit;
        let mut sink = BatchSink::new(&mut emit, started, limit);
        if request.query.trim().is_empty() {
            sink.finish();
            return Ok(());
        }
        let roots = if request.root == "everywhere://" {
            accessible_roots()
        } else {
            vec![request.root.clone()]
        };
        for root in roots {
            if cancelled() {
                return Ok(());
            }
            if let Err(error) = search_root(&root, request, &cancelled, &mut sink) {
                if matches!(request.engine, SearchEngine::Auto) {
                    // Auto never silently merges engines: record why the
                    // preferred engine was skipped and keep the builtin
                    // fallback for the remaining roots.
                    sink.warning = Some(error);
                } else {
                    return Err(error);
                }
            }
        }
        sink.finish();
        Ok(())
    }
}

#[cfg(test)]
mod tests;
