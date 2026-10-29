//! Live filename search backed by ripgrep's directory walker.
//!
//! There is no persistent index: each query walks the current filesystem state
//! with `ignore::WalkBuilder`. Advanced indexed/semantic search belongs in a
//! plugin through `extensions.search`.
use crate::{
    core::files::{entry, Entry},
    core::ignores::IgnoreSet,
};
use ignore::WalkBuilder;
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};

const RESULT_LIMIT: usize = 500;

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

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchBatch {
    pub entries: Vec<Entry>,
    pub done: bool,
    pub elapsed_ms: u128,
    pub truncated: bool,
    pub skipped: usize,
}

/// Emit bounded batches during the walk. The callback must not retain a whole
/// search's results; the UI owns accumulated hits for its current pane.
pub(crate) fn search_local_stream(
    root: &Path,
    query: &str,
    mode: &str,
    hidden: bool,
    ignores: &[String],
    cancelled: &dyn Fn() -> bool,
    emit: impl Fn(SearchBatch),
) -> Result<(), String> {
    if !matches!(mode, "name" | "content") { return Err("invalid search mode".into()); }
    if query.trim().is_empty() { return Ok(()); }
    let started = Instant::now();
    let mut batch = Vec::new();
    let mut total = 0;
    let mut skipped = 0;
    let mut bytes_read = 0usize;
    let mut truncated = false;
    let ignore = Arc::new(IgnoreSet::new(ignores));
    let filter_root = root.to_path_buf();
    let walker = WalkBuilder::new(root)
        .hidden(!hidden).ignore(false).git_ignore(false).git_global(false)
        .git_exclude(false).parents(false).follow_links(false).same_file_system(true)
        .filter_entry(move |item| item.depth() == 0 ||
            (!inside_bundle(item.path()) && !ignore.is_ignored(
                item.path().strip_prefix(&filter_root).unwrap_or(item.path()))))
        .build();
    let needle = query.to_lowercase();
    for item in walker {
        if cancelled() { return Ok(()); }
        let item = match item {
            Ok(item) if item.depth() > 0 => item,
            Ok(_) => continue,
            Err(_) => { skipped += 1; continue; }
        };
        let matched = if mode == "name" {
            item.file_name().to_string_lossy().to_lowercase().contains(&needle)
        } else {
            if item.file_type().is_some_and(|kind| kind.is_symlink()) { continue; }
            let Ok(meta) = item.metadata() else { skipped += 1; continue; };
            if !meta.is_file() { continue; }
            if meta.len() > 2 * 1024 * 1024 { truncated = true; continue; }
            if bytes_read + meta.len() as usize > 32 * 1024 * 1024 {
                truncated = true;
                break;
            }
            // Read only local regular files under the already authorized root.
            let Ok(bytes) = std::fs::read(item.path()) else { skipped += 1; continue; };
            bytes_read += bytes.len();
            !bytes.contains(&0) && String::from_utf8(bytes)
                .is_ok_and(|text| text.to_lowercase().contains(&needle))
        };
        if !matched { continue; }
        match entry(item.path()) {
            Ok(found) => { batch.push(found); total += 1; },
            Err(_) => { skipped += 1; continue; }
        }
        if batch.len() >= 50 {
            emit(SearchBatch { entries: std::mem::take(&mut batch), done: false,
                elapsed_ms: started.elapsed().as_millis(), truncated: false, skipped: 0 });
        }
        if total >= RESULT_LIMIT { truncated = true; break; }
    }
    if !cancelled() {
        emit(SearchBatch { entries: batch, done: true,
            elapsed_ms: started.elapsed().as_millis(), truncated, skipped });
    }
    Ok(())
}

/// Explicitly budgeted content fallback for plugin mounts. Every directory
/// and file is resolved through the same Core broker as ordinary FS actions.
pub(crate) fn search_vfs_content_stream(
    root: &str,
    query: &str,
    hidden: bool,
    ignores: &[String],
    cancelled: &dyn Fn() -> bool,
    emit: impl Fn(SearchBatch),
) -> Result<(), String> {
    const MAX_DIRS: usize = 100;
    const MAX_FILES: usize = 100;
    const MAX_BYTES: usize = 16 * 1024 * 1024;
    const PER_FILE: usize = 1024 * 1024;
    let started = Instant::now();
    let mut pending = vec![root.to_string()];
    let mut seen = HashSet::new();
    let mut batch = Vec::new();
    let mut scanned = 0;
    let mut dirs = 0;
    let mut bytes_read = 0;
    let mut skipped = 0;
    let mut truncated = false;
    let needle = query.to_lowercase();
    let ignore = IgnoreSet::new(ignores);
    while let Some(directory) = pending.pop() {
        if cancelled() { return Ok(()); }
        if !seen.insert(directory.clone()) { continue; }
        if dirs >= MAX_DIRS { truncated = true; break; }
        dirs += 1;
        let parent = crate::plugins::mounts::parse(&directory)?;
        let broker = crate::core::fs::trusted(&directory, "core:search")?;
        let listing = broker.desktop_listing(hidden, ignores)?;
        for item in listing.entries {
            if cancelled() { return Ok(()); }
            let name = &item.name;
            crate::plugins::mounts::segment(name)?;
            if !hidden && name.starts_with('.') { continue; }
            let relative = if parent.relative.is_empty() { name.to_owned() }
                else { format!("{}/{}", parent.relative, name) };
            if ignore.is_ignored(Path::new(&relative)) { continue; }
            let path = crate::plugins::mounts::uri(&parent.mount_id, &relative)?;
            if item.path != path {
                return Err("VFS listing returned a child outside its directory".into());
            }
            if item.is_dir { pending.push(path); continue; }
            if item.is_symlink { continue; }
            if item.size > PER_FILE as u64 { truncated = true; continue; }
            if scanned >= MAX_FILES || bytes_read >= MAX_BYTES {
                truncated = true;
                break;
            }
            scanned += 1;
            let file = crate::core::fs::trusted(&path, "core:search")?;
            let bytes = match file.preview_prefix(PER_FILE.min(MAX_BYTES - bytes_read)) {
                Ok(bytes) => bytes,
                Err(_) => { skipped += 1; continue; }
            };
            bytes_read += bytes.len();
            if bytes.contains(&0) { continue; }
            if String::from_utf8(bytes).is_ok_and(|text| text.to_lowercase().contains(&needle)) {
                batch.push(item);
                if batch.len() >= 50 {
                    emit(SearchBatch { entries: std::mem::take(&mut batch), done: false,
                        elapsed_ms: started.elapsed().as_millis(), truncated: false, skipped: 0 });
                }
            }
        }
        if truncated { break; }
    }
    if !cancelled() {
        emit(SearchBatch { entries: batch, done: true,
            elapsed_ms: started.elapsed().as_millis(), truncated, skipped });
    }
    Ok(())
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

    pub fn run_global(
        &self,
        scope: &str,
        query: &str,
        hidden: bool,
        ignores: &[String],
        emit: impl Fn(SearchBatch) + Send + Sync,
    ) -> Result<(), String> {
        let started = Instant::now();
        let generation = self.generation(scope);
        let ticket = generation.fetch_add(1, Ordering::SeqCst) + 1;
        let mut entries = Vec::new();
        let mut seen = HashSet::new();
        let mut truncated = false;
        let mut skipped = 0usize;
        for root in accessible_roots() {
            if generation.load(Ordering::Relaxed) != ticket || entries.len() >= RESULT_LIMIT {
                break;
            }
            let (found, root_truncated, root_skipped) = search_root(
                Path::new(&root),
                query,
                hidden,
                ignores,
                &generation,
                ticket,
                RESULT_LIMIT.saturating_sub(entries.len()),
            );
            truncated |= root_truncated;
            skipped += root_skipped;
            let found = crate::plugins::resolvers::search::resolve(
                &crate::plugins::resolvers::search::PluginSearchInput {
                    query: query.into(),
                    mode: "name".into(),
                    engine: "auto".into(),
                    root: root.clone(),
                    hidden,
                    ignores: ignores.to_vec(),
                    limit: Some(RESULT_LIMIT.saturating_sub(entries.len())),
                },
                || Ok(serde_json::json!({"entries":found})),
                || generation.load(Ordering::Relaxed) != ticket,
            );
            for item in found {
                if entries.len() >= RESULT_LIMIT {
                    truncated = true;
                    break;
                }
                if seen.insert(item.path.clone()) {
                    entries.push(item);
                }
            }
        }
        let truncated = truncated || entries.len() >= RESULT_LIMIT;
        emit(SearchBatch {
            entries,
            done: true,
            elapsed_ms: started.elapsed().as_millis(),
            truncated,
            skipped,
        });
        Ok(())
    }

    pub fn run(
        &self,
        scope: &str,
        root: &str,
        query: &str,
        hidden: bool,
        ignores: &[String],
        emit: impl Fn(SearchBatch) + Send + Sync,
    ) -> Result<(), String> {
        let started = Instant::now();
        let generation = self.generation(scope);
        let ticket = generation.fetch_add(1, Ordering::SeqCst) + 1;
        let (entries, truncated, skipped) = search_root(
            Path::new(root),
            query,
            hidden,
            ignores,
            &generation,
            ticket,
            RESULT_LIMIT,
        );
        let entries = crate::plugins::resolvers::search::resolve(
            &crate::plugins::resolvers::search::PluginSearchInput {
                query: query.into(),
                mode: "name".into(),
                engine: "auto".into(),
                root: root.into(),
                hidden,
                ignores: ignores.to_vec(),
                limit: Some(RESULT_LIMIT),
            },
            || Ok(serde_json::json!({"entries":entries})),
            || generation.load(Ordering::Relaxed) != ticket,
        );
        if generation.load(Ordering::Relaxed) != ticket {
            return Ok(());
        }
        let truncated = truncated || entries.len() >= RESULT_LIMIT;
        emit(SearchBatch {
            entries,
            done: true,
            elapsed_ms: started.elapsed().as_millis(),
            truncated,
            skipped,
        });
        Ok(())
    }
}

fn search_root(
    root: &Path,
    query: &str,
    hidden: bool,
    ignores: &[String],
    generation: &AtomicU64,
    ticket: u64,
    limit: usize,
) -> (Vec<Entry>, bool, usize) {
    search_directory(root, query, hidden, ignores, limit, &|| {
        generation.load(Ordering::Relaxed) != ticket
    })
}

/// Adapter-neutral filename search. The caller owns cancellation and receives
/// bounded results plus the number of unreadable/ignored entries.
pub(crate) fn search_directory(
    root: &Path,
    query: &str,
    hidden: bool,
    ignores: &[String],
    limit: usize,
    cancelled: &dyn Fn() -> bool,
) -> (Vec<Entry>, bool, usize) {
    if limit == 0 || !root.is_dir() {
        return (Vec::new(), false, 0);
    }
    let lowered = query.to_lowercase();
    if lowered.is_empty() {
        return (Vec::new(), false, 0);
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
    let mut entries = Vec::new();
    let mut skipped = 0usize;
    let mut truncated = false;
    for item in walker {
        if cancelled() {
            return (Vec::new(), false, 0);
        }
        match item {
            Ok(item) if item.depth() > 0 => {
                let name = item.file_name().to_string_lossy().to_lowercase();
                if !name.contains(&lowered) {
                    continue;
                }
                if let Ok(entry) = entry(item.path()) {
                    entries.push(entry);
                } else {
                    skipped += 1;
                }
                if entries.len() >= limit {
                    truncated = true;
                    break;
                }
            }
            Err(_) => skipped += 1,
            _ => {}
        }
    }
    (entries, truncated, skipped)
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
