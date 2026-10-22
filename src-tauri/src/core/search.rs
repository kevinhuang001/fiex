use crate::{
    files::{entry, Entry},
    ignores::IgnoreSet,
};
use ignore::WalkBuilder;
use notify::Watcher;
use serde::Serialize;
use std::{
    collections::{hash_map::DefaultHasher, HashMap, HashSet},
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};
use tantivy::tokenizer::{LowerCaser, RawTokenizer, TextAnalyzer};
use tantivy::{
    collector::DocSetCollector,
    indexer::IndexWriter,
    query::RegexQuery,
    schema::{
        Field, IndexRecordOption, Schema, TantivyDocument, Term, TextFieldIndexing, TextOptions,
        Value,
    },
    Index, IndexReader, ReloadPolicy,
};

const RESULT_LIMIT: usize = 500;
const INDEX_LIMIT: usize = 1_000_000;
// A burst larger than this is cheaper to rebuild than to patch file by file.
const PENDING_LIMIT: usize = 20_000;
const INDEX_VERSION: u32 = 3;

#[derive(Serialize, serde::Deserialize)]
struct IndexMeta {
    version: u32,
    root: String,
    signature: String,
}

struct IndexHandle {
    root: String,
    signature: String,
    _index: Index,
    reader: IndexReader,
    writer: IndexWriter,
    name: Field,
    path: Field,
    path_id: Field,
    skipped: usize,
    // Files touched since the last search. Applying them incrementally keeps a
    // busy home folder from rebuilding the whole index on every keystroke.
    pending: Arc<Mutex<HashSet<PathBuf>>>,
    overflow: Arc<AtomicBool>,
    _watcher: Option<notify::RecommendedWatcher>,
}

#[derive(Clone, Default)]
pub struct SearchState {
    generations: Arc<Mutex<HashMap<String, Arc<AtomicU64>>>>,
    index: Arc<Mutex<Option<IndexHandle>>>,
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

fn signature(hidden: bool, ignores: &[String]) -> String {
    format!(
        "v{INDEX_VERSION}:hidden={hidden}:{}",
        ignores.join("\u{1f}")
    )
}

fn index_dir(root: &str, signature: &str) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    root.hash(&mut hasher);
    signature.hash(&mut hasher);
    let base = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".fiex")
        .join("index");
    base.join(format!("{:016x}", hasher.finish()))
}

fn index_base() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".fiex")
        .join("index")
}

fn open_index(dir: &Path, root: &str, signature: &str) -> Option<IndexHandle> {
    let meta: IndexMeta =
        serde_json::from_slice(&fs::read(dir.join("fiex-meta.json")).ok()?).ok()?;
    if meta.version != INDEX_VERSION || meta.root != root || meta.signature != signature {
        return None;
    }
    let index = Index::open_in_dir(dir).ok()?;
    index.tokenizers().register(
        "filename",
        TextAnalyzer::builder(RawTokenizer::default())
            .filter(LowerCaser)
            .build(),
    );
    index
        .tokenizers()
        .register("path_id", RawTokenizer::default());
    let reader: IndexReader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::Manual)
        .try_into()
        .ok()?;
    reader.reload().ok()?;
    let schema = index.schema();
    let name = schema.get_field("name").ok()?;
    let path = schema.get_field("path").ok()?;
    let path_id = schema.get_field("path_id").ok()?;
    let writer = index
        .writer_with_num_threads::<TantivyDocument>(1, 50_000_000)
        .ok()?;
    let pending = Arc::new(Mutex::new(HashSet::new()));
    let overflow = Arc::new(AtomicBool::new(false));
    let watcher = watch_root(root, pending.clone(), overflow.clone());
    Some(IndexHandle {
        root: root.into(),
        signature: signature.into(),
        _index: index,
        reader,
        writer,
        name,
        path,
        path_id,
        skipped: 0,
        pending,
        overflow,
        _watcher: watcher,
    })
}

fn watch_root(
    root: &str,
    pending: Arc<Mutex<HashSet<PathBuf>>>,
    overflow: Arc<AtomicBool>,
) -> Option<notify::RecommendedWatcher> {
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        let Ok(event) = event else { return };
        if !matches!(
            event.kind,
            notify::EventKind::Create(_)
                | notify::EventKind::Modify(_)
                | notify::EventKind::Remove(_)
        ) {
            return;
        }
        let mut queue = pending.lock().unwrap();
        if queue.len() + event.paths.len() > PENDING_LIMIT {
            queue.clear();
            overflow.store(true, Ordering::SeqCst);
            return;
        }
        queue.extend(event.paths);
    })
    .ok()?;
    watcher
        .watch(Path::new(root), notify::RecursiveMode::Recursive)
        .ok()?;
    Some(watcher)
}

fn build_index(
    dir: &Path,
    root: &str,
    signature: &str,
    hidden: bool,
    ignores: &[String],
) -> Result<IndexHandle, String> {
    if dir.exists() {
        fs::remove_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;

    let mut schema_builder = Schema::builder();
    let name = schema_builder.add_text_field(
        "name",
        TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("filename")
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions),
            )
            .set_stored(),
    );
    let path = schema_builder.add_text_field("path", TextOptions::default().set_stored());
    let path_id = schema_builder.add_text_field(
        "path_id",
        TextOptions::default().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("path_id")
                .set_index_option(IndexRecordOption::Basic),
        ),
    );
    let schema = schema_builder.build();
    let index = Index::create_in_dir(dir, schema).map_err(|e| e.to_string())?;
    index.tokenizers().register(
        "filename",
        TextAnalyzer::builder(RawTokenizer::default())
            .filter(LowerCaser)
            .build(),
    );
    index
        .tokenizers()
        .register("path_id", RawTokenizer::default());
    let mut writer = index
        .writer_with_num_threads::<TantivyDocument>(1, 50_000_000)
        .map_err(|e| e.to_string())?;

    let root_path = PathBuf::from(root);
    let filter_root = root_path.clone();
    let filter_ignore = Arc::new(IgnoreSet::new(ignores));
    let filter_index = index_base();
    let mut skipped = 0usize;
    let mut added = 0usize;
    let walker = WalkBuilder::new(root)
        .hidden(!hidden)
        .ignore(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .parents(false)
        .follow_links(false)
        .threads(1)
        .filter_entry(move |item| {
            if item.depth() == 0 {
                return true;
            }
            let relative = item
                .path()
                .strip_prefix(&filter_root)
                .unwrap_or(item.path());
            !item.path().starts_with(&filter_index)
                && !inside_bundle(item.path())
                && !filter_ignore.is_ignored(relative)
        })
        .build();
    for result in walker {
        match result {
            Ok(item) if item.depth() > 0 => {
                if added >= INDEX_LIMIT {
                    break;
                }
                if !add_document(
                    &writer,
                    IndexFields {
                        name,
                        path,
                        path_id,
                    },
                    item.path(),
                ) {
                    skipped += 1;
                } else {
                    added += 1;
                }
            }
            Err(_) => skipped += 1,
            _ => {}
        }
    }
    writer.commit().map_err(|e| e.to_string())?;
    fs::write(
        dir.join("fiex-meta.json"),
        serde_json::to_vec(&IndexMeta {
            version: INDEX_VERSION,
            root: root.into(),
            signature: signature.into(),
        })
        .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let reader: IndexReader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::Manual)
        .try_into()
        .map_err(|e: tantivy::TantivyError| e.to_string())?;
    reader.reload().map_err(|e| e.to_string())?;
    let pending = Arc::new(Mutex::new(HashSet::new()));
    let overflow = Arc::new(AtomicBool::new(false));
    let watcher = watch_root(root, pending.clone(), overflow.clone());
    Ok(IndexHandle {
        root: root.into(),
        signature: signature.into(),
        _index: index,
        reader,
        writer,
        name,
        path,
        path_id,
        skipped,
        pending,
        overflow,
        _watcher: watcher,
    })
}

#[derive(Clone, Copy)]
struct IndexFields {
    name: Field,
    path: Field,
    path_id: Field,
}

fn add_document(writer: &IndexWriter, fields: IndexFields, item: &Path) -> bool {
    let mut doc = TantivyDocument::new();
    doc.add_text(
        fields.name,
        item.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase(),
    );
    doc.add_text(fields.path, item.to_string_lossy());
    doc.add_text(fields.path_id, item.to_string_lossy());
    writer.add_document(doc).is_ok()
}

/// Re-index one path (a file or a whole subtree) after a filesystem event.
fn index_subtree(
    writer: &IndexWriter,
    fields: IndexFields,
    target: &Path,
    root: &Path,
    hidden: bool,
    ignores: &[String],
) -> usize {
    let base = index_base();
    let filter_root = root.to_path_buf();
    let filter_ignore = Arc::new(IgnoreSet::new(ignores));
    let mut added = 0usize;
    let walker = WalkBuilder::new(target)
        .hidden(!hidden)
        .ignore(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .parents(false)
        .follow_links(false)
        .threads(1)
        .filter_entry(move |item| {
            !item.path().starts_with(&base)
                && !inside_bundle(item.path())
                && !filter_ignore.is_ignored(
                    item.path()
                        .strip_prefix(&filter_root)
                        .unwrap_or(item.path()),
                )
        })
        .build();
    for item in walker.flatten() {
        add_document(writer, fields, item.path());
        added += 1;
    }
    added
}

fn apply_pending(handle: &mut IndexHandle, hidden: bool, ignores: &[String]) -> Result<(), String> {
    let mut paths: Vec<PathBuf> = {
        let mut queue = handle.pending.lock().unwrap();
        if queue.is_empty() {
            return Ok(());
        }
        queue.drain().collect()
    };
    // A rename can report both the directory and every child. Process the
    // ancestor once, so a large move does not re-index the same tree N times.
    paths.sort_by_key(|path| path.components().count());
    let mut roots: Vec<PathBuf> = Vec::new();
    for path in paths {
        if !roots.iter().any(|root| path.starts_with(root)) {
            roots.push(path);
        }
    }
    let root = PathBuf::from(&handle.root);
    let searcher = handle.reader.searcher();
    for target in roots {
        // Remove descendants too. A deleted or renamed directory no longer
        // exists on disk, so walking it cannot discover its old index entries.
        let prefix = format!("{}.*", regex::escape(&target.to_string_lossy()));
        let query =
            RegexQuery::from_pattern(&prefix, handle.path_id).map_err(|error| error.to_string())?;
        let addresses = searcher
            .search(&query, &DocSetCollector)
            .map_err(|error| error.to_string())?;
        for address in addresses {
            if let Ok(doc) = searcher.doc::<TantivyDocument>(address) {
                if let Some(path) = doc.get_first(handle.path).and_then(|value| value.as_str()) {
                    if Path::new(path).starts_with(&target) {
                        handle
                            .writer
                            .delete_term(Term::from_field_text(handle.path_id, path));
                    }
                }
            }
        }
        if target.exists() {
            index_subtree(
                &handle.writer,
                IndexFields {
                    name: handle.name,
                    path: handle.path,
                    path_id: handle.path_id,
                },
                &target,
                &root,
                hidden,
                ignores,
            );
        }
    }
    handle.writer.commit().map_err(|e| e.to_string())?;
    handle.reader.reload().map_err(|e| e.to_string())?;
    Ok(())
}

fn ensure_index(
    handle: &mut Option<IndexHandle>,
    root: &str,
    hidden: bool,
    ignores: &[String],
) -> Result<(), String> {
    let signature = signature(hidden, ignores);
    let switched = match handle.as_ref() {
        Some(current) => current.root != root || current.signature != signature,
        None => true,
    };
    let flooded = handle
        .as_ref()
        .is_some_and(|current| current.overflow.swap(false, Ordering::SeqCst));
    if !switched && !flooded {
        if let Some(current) = handle.as_mut() {
            apply_pending(current, hidden, ignores)?;
        }
        return Ok(());
    }
    let rebuild = switched || flooded;
    let dir = index_dir(root, &signature);
    fs::create_dir_all(dir.parent().ok_or("无效索引目录")?).map_err(|e| e.to_string())?;
    let opened = if handle.is_some() {
        // A dirty index is rebuilt from scratch; a different root may reuse its
        // persisted index if the metadata matches.
        if handle
            .as_ref()
            .is_some_and(|current| current.root == root && current.signature == signature)
        {
            None
        } else {
            open_index(&dir, root, &signature)
        }
    } else {
        open_index(&dir, root, &signature)
    };
    if rebuild {
        if let Some(opened) = opened {
            *handle = Some(opened);
        } else {
            *handle = Some(build_index(&dir, root, &signature, hidden, ignores)?);
        }
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

    pub fn warm(&self, root: &str, hidden: bool, ignores: &[String]) -> Result<(), String> {
        let mut guard = self.index.lock().unwrap();
        ensure_index(&mut guard, root, hidden, ignores)
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
        let counter = self.generation(scope);
        let generation = counter.fetch_add(1, Ordering::SeqCst) + 1;
        let started = Instant::now();
        let query = query.to_lowercase();
        let mut guard = self.index.lock().unwrap();
        ensure_index(&mut guard, root, hidden, ignores)?;
        let handle = guard.as_ref().ok_or("索引不可用")?;
        let searcher = handle.reader.searcher();
        let pattern = format!(".*{}.*", regex::escape(&query));
        let regex_query =
            RegexQuery::from_pattern(&pattern, handle.name).map_err(|e| e.to_string())?;
        // Scores are irrelevant for substring matching, so collect plain
        // addresses instead of ranking every match.
        let found = searcher
            .search(&regex_query, &DocSetCollector)
            .map_err(|e| e.to_string())?;
        let truncated = found.len() >= RESULT_LIMIT;
        let mut entries = Vec::new();
        for address in found.into_iter().take(RESULT_LIMIT) {
            if counter.load(Ordering::Relaxed) != generation {
                return Ok(());
            }
            if let Ok(doc) = searcher.doc::<TantivyDocument>(address) {
                if let Some(path) = doc.get_first(handle.path).and_then(|value| value.as_str()) {
                    if let Ok(entry) = entry(Path::new(path)) {
                        entries.push(entry);
                    }
                }
            }
        }
        emit(SearchBatch {
            entries,
            done: true,
            elapsed_ms: started.elapsed().as_millis(),
            truncated,
            skipped: handle.skipped,
        });
        Ok(())
    }
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
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn finds_app_bundles_without_entering_them() {
        let t = tempfile::tempdir().unwrap();
        let bundle = t.path().join("Safari.app");
        std::fs::create_dir_all(bundle.join("Contents")).unwrap();
        std::fs::write(bundle.join("Contents").join("Needle.plist"), "x").unwrap();
        std::fs::write(t.path().join("Needle.txt"), "x").unwrap();

        let state = SearchState::default();
        let found = Mutex::new(Vec::new());
        state
            .run(
                "test",
                t.path().to_str().unwrap(),
                "needle",
                false,
                &[],
                |batch| found.lock().unwrap().extend(batch.entries),
            )
            .unwrap();
        assert_eq!(found.lock().unwrap().len(), 1);
        assert_eq!(found.lock().unwrap()[0].name, "Needle.txt");

        let apps = Mutex::new(Vec::new());
        state
            .run(
                "test",
                t.path().to_str().unwrap(),
                "safari",
                false,
                &[],
                |batch| apps.lock().unwrap().extend(batch.entries),
            )
            .unwrap();
        let apps = apps.lock().unwrap();
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "Safari.app");
        assert!(apps[0].is_dir);
    }

    #[test]
    fn finds_hidden_when_requested() {
        let t = tempfile::tempdir().unwrap();
        std::fs::write(t.path().join(".Needle.txt"), "x").unwrap();
        let state = SearchState::default();
        let out = Mutex::new(Vec::new());
        state
            .run(
                "test",
                t.path().to_str().unwrap(),
                "needle",
                true,
                &[],
                |b| out.lock().unwrap().extend(b.entries),
            )
            .unwrap();
        assert_eq!(out.lock().unwrap().len(), 1);
    }

    #[test]
    fn display_ignores_filter_results() {
        let t = tempfile::tempdir().unwrap();
        std::fs::create_dir(t.path().join("node_modules")).unwrap();
        std::fs::write(t.path().join("node_modules").join("Needle.js"), "x").unwrap();
        std::fs::write(t.path().join("Needle.txt"), "x").unwrap();
        let state = SearchState::default();
        let out = Mutex::new(Vec::new());
        state
            .run(
                "test",
                t.path().to_str().unwrap(),
                "needle",
                false,
                &["node_modules".into()],
                |b| out.lock().unwrap().extend(b.entries),
            )
            .unwrap();
        assert_eq!(out.lock().unwrap().len(), 1);
        assert_eq!(out.lock().unwrap()[0].name, "Needle.txt");
    }

    #[test]
    fn removing_a_directory_removes_its_indexed_descendants() {
        let t = tempfile::tempdir().unwrap();
        let folder = t.path().join("OldFolder");
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("Needle.txt"), "x").unwrap();
        let state = SearchState::default();
        state.warm(t.path().to_str().unwrap(), false, &[]).unwrap();
        fs::remove_dir_all(&folder).unwrap();
        {
            let guard = state.index.lock().unwrap();
            guard
                .as_ref()
                .unwrap()
                .pending
                .lock()
                .unwrap()
                .insert(folder);
        }
        let matches = Mutex::new(Vec::new());
        state
            .run(
                "test",
                t.path().to_str().unwrap(),
                "needle",
                false,
                &[],
                |batch| {
                    matches.lock().unwrap().extend(batch.entries);
                },
            )
            .unwrap();
        assert!(matches.lock().unwrap().is_empty());
        let guard = state.index.lock().unwrap();
        let handle = guard.as_ref().unwrap();
        let searcher = handle.reader.searcher();
        let query = RegexQuery::from_pattern(".*Needle.*", handle.path_id).unwrap();
        assert!(searcher
            .search(&query, &DocSetCollector)
            .unwrap()
            .is_empty());
    }
}
