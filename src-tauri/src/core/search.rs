//! Live search with independent filename/content modes.
//!
//! Local FS queries can be served by an installed `SearchProvider` (for example
//! Everything) or by the builtin walker, which is also the last Auto candidate.
//! Only one engine runs per query; Core validates every hit, deduplicates it and
//! streams partial batches to the UI before the walk finishes.
use crate::{
    core::files::{entry, Entry},
    core::ignores::IgnoreSet,
};
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

pub const RESULT_LIMIT: usize = 500;
const BATCH_SIZE: usize = 50;
const BATCH_INTERVAL: Duration = Duration::from_millis(120);
/// Content fallback stays local and bounded: no PDF/Office extraction and no
/// unbounded reads, so a keystroke can never walk a whole disk byte by byte.
const CONTENT_FILE_LIMIT: u64 = 2 * 1024 * 1024;
const CONTENT_BYTE_BUDGET: u64 = 64 * 1024 * 1024;
const CONTENT_FILE_BUDGET: usize = 5000;
const BINARY_PROBE: usize = 8192;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchMode {
    #[default]
    Name,
    Content,
}

impl SearchMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Content => "content",
        }
    }
}

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
    emit: &'a (dyn Fn(SearchBatch) + Send + Sync),
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
    fn new(emit: &'a (dyn Fn(SearchBatch) + Send + Sync), started: Instant, limit: usize) -> Self {
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

    fn extend(&mut self, entries: Vec<Entry>) {
        for entry in entries {
            self.push(entry);
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
        let mut sink = BatchSink::new(&emit, started, limit);
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

fn search_root(
    root: &str,
    request: &SearchRequest,
    cancelled: &(dyn Fn() -> bool + Sync),
    sink: &mut BatchSink<'_>,
) -> Result<(), String> {
    // Core-owned virtual roots are not host paths: filter their root listing
    // instead of asking a filesystem SearchProvider about a URI.
    if virtual_search_root(root)
        || (request.mode == SearchMode::Name && crate::plugins::mounts::handles(root))
    {
        return search_cached_vfs(root, request, cancelled, sink);
    }
    let local = !crate::plugins::mounts::handles(root) && !root.contains("://");
    if !local {
        return search_adapter(root, request, cancelled, sink);
    }
    let input = crate::plugins::resolvers::search::PluginSearchInput {
        query: request.query.clone(),
        root: root.to_string(),
        hidden: request.hidden,
        mode: request.mode,
        ignores: request.ignores.clone(),
        limit: Some(sink.limit),
    };
    let engines = crate::plugins::resolvers::search::engines(&input);
    let selected = match &request.engine {
        SearchEngine::Builtin => None,
        SearchEngine::Provider(id) => {
            let engine = engines
                .iter()
                .find(|engine| !engine.is_builtin() && engine.resolver.id == *id)
                .ok_or_else(|| format!("未知搜索提供程序：{id}"))?;
            if !engine.supports(request.mode) {
                return Err(format!(
                    "{} 不支持{}搜索",
                    engine.resolver.name,
                    mode_label(request.mode)
                ));
            }
            Some(engine.clone())
        }
        SearchEngine::Auto => engines
            .iter()
            .find(|engine| !engine.is_builtin() && engine.supports(request.mode))
            .cloned(),
    };
    if let Some(engine) = selected {
        match crate::plugins::resolvers::search::run(&engine, &input, cancelled) {
            Ok((entries, truncated)) => {
                sink.engine = Some(engine.resolver.name.clone());
                sink.truncated |= truncated;
                sink.extend(entries);
                return Ok(());
            }
            Err(error) => {
                if matches!(request.engine, SearchEngine::Auto) {
                    sink.warning = Some(format!("{}: {error}", engine.resolver.name));
                } else {
                    return Err(error);
                }
            }
        }
    }
    sink.engine = Some("builtin".into());
    walk_root(
        Path::new(root),
        &request.query,
        request.mode,
        request.hidden,
        &request.ignores,
        cancelled,
        sink,
    );
    Ok(())
}

/// A virtual search root owns a cached subtree (a root or one of its
/// subdirectories). Only cached directories are searched; uncached directories
/// are never enumerated.
fn virtual_search_root(root: &str) -> bool {
    if crate::plugins::mounts::handles(root) {
        return false;
    }
    crate::core::volumes::virtual_storage::is_virtual_tree_path(root)
}

fn search_cached_vfs(
    root: &str,
    request: &SearchRequest,
    cancelled: &(dyn Fn() -> bool + Sync),
    sink: &mut BatchSink<'_>,
) -> Result<(), String> {
    sink.engine = Some("builtin".into());
    if request.mode == SearchMode::Content {
        // There is no file content to read behind a virtual identity. Return an
        // explicit warning instead of a silently empty result.
        sink.warning =
            Some(crate::i18n::t("暂不支持在“最近使用”“应用程序”和“废纸篓”中搜索文件内容").into());
        sink.incomplete = true;
        return Ok(());
    }
    let mut cached = Vec::new();
    crate::core::fs::directory_cache::search(
        root,
        &request.query,
        request.hidden,
        &request.ignores,
        cancelled,
        &mut |entry| {
            if cached.len() < sink.limit {
                cached.push(entry.clone());
                sink.push(entry);
            }
        },
    )?;
    sink.flush(false);
    if cancelled() {
        return Ok(());
    }
    // The live pass emits additions/metadata updates as they arrive; old hits
    // stay visible until the final authoritative set can remove stale hits.
    let live: Mutex<Vec<Entry>> = Mutex::new(Vec::new());
    let upstream = sink.emit;
    let emit = |mut batch: SearchBatch| {
        {
            let mut live = live.lock().unwrap_or_else(|e| e.into_inner());
            for entry in &batch.entries {
                if let Some(previous) = live.iter_mut().find(|previous| previous.path == entry.path)
                {
                    *previous = entry.clone();
                } else {
                    live.push(entry.clone());
                }
            }
        }
        batch
            .entries
            .retain(|entry| !cached.iter().any(|old| same_entry(old, entry)));
        batch.done = false;
        batch.replace = false;
        if !batch.entries.is_empty() {
            (upstream)(batch);
        }
    };
    let mut fresh = BatchSink::new(&emit, sink.started, sink.limit);
    let result = search_adapter(root, request, cancelled, &mut fresh);
    fresh.flush(false);
    if cancelled() {
        return Ok(());
    }
    if let Err(error) = result {
        sink.warning = Some(error);
        sink.incomplete = true;
        sink.emit_snapshot(Vec::new(), true, false, false, 0, 0);
        return Ok(());
    }
    let entries = live.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let same = same_entry_snapshot(&cached, &entries);
    let truncated = fresh.truncated;
    let skipped = fresh.skipped;
    let scanned = fresh.scanned;
    drop(fresh);
    sink.emit_snapshot(
        if same { Vec::new() } else { entries },
        true,
        !same,
        truncated,
        skipped,
        scanned,
    );
    Ok(())
}
fn same_entry(left: &Entry, right: &Entry) -> bool {
    serde_json::to_value(left).ok() == serde_json::to_value(right).ok()
}
fn same_entry_snapshot(left: &[Entry], right: &[Entry]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .all(|left| right.iter().any(|right| same_entry(left, right)))
}

fn search_adapter(
    root: &str,
    request: &SearchRequest,
    cancelled: &dyn Fn() -> bool,
    sink: &mut BatchSink<'_>,
) -> Result<(), String> {
    let resource = crate::core::fs::trusted(root, "core:search")?;
    if request.mode == SearchMode::Content {
        // Remote content fallback must confirm a network budget first; until
        // that flow exists, refuse instead of silently downloading content.
        return Err("暂不支持在远程连接中搜索文件内容".into());
    }
    sink.engine = Some("builtin".into());
    let limit = sink.limit;
    let (truncated, skipped) = resource.search_stream(
        &request.query,
        request.hidden,
        &request.ignores,
        limit,
        cancelled,
        &mut |entry| sink.push(entry),
    )?;
    sink.truncated |= truncated;
    sink.skipped += skipped;
    Ok(())
}

fn mode_label(mode: SearchMode) -> &'static str {
    match mode {
        SearchMode::Name => "文件名",
        SearchMode::Content => "文件内容",
    }
}

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
        let emit = |batch: SearchBatch| {
            if let Ok(mut found) = found.lock() {
                found.extend(batch.entries);
            }
        };
        let mut sink = BatchSink::new(&emit, Instant::now(), limit);
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

/// Only filesystem roots readable with the application's own user token are
/// scanned; protected paths naturally fail and are recorded as skipped.
pub fn accessible_roots() -> Vec<String> {
    let home = dirs::home_dir().filter(|path| path.is_dir());
    let mut roots = Vec::new();
    for location in crate::core::volumes::locations() {
        if location.kind != "disk"
            || !location.mounted
            || location.usbfs
            || location.path.contains("://")
        {
            continue;
        }
        let path = PathBuf::from(location.path);
        if !path.is_dir() || std::fs::read_dir(&path).is_err() {
            continue;
        }
        // Searching `/` (or the Windows system drive) would traverse the
        // whole OS for every keystroke. Prefer the user's home directory and
        // only add independent mounts outside it.
        if home.as_ref().is_some_and(|home| home.starts_with(&path)) {
            continue;
        }
        roots.push(path);
    }
    if let Some(home) = home {
        roots.push(home);
    }
    roots.sort();
    roots.dedup();
    roots
        .into_iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
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

#[cfg(test)]
mod tests;
