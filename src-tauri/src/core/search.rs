//! Live filename search backed by ripgrep's directory walker.
//!
//! There is no persistent index: each query walks the current filesystem state
//! with `ignore::WalkBuilder`. Advanced indexed/semantic search belongs in a
//! plugin through `extensions.search`.
use crate::{
    files::{entry, Entry},
    ignores::IgnoreSet,
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

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchBatch {
    pub entries: Vec<Entry>,
    pub done: bool,
    pub elapsed_ms: u128,
    pub truncated: bool,
    pub skipped: usize,
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
            let found = crate::plugin_search::resolve(
                &crate::plugin_search::PluginSearchInput {
                    query: query.into(),
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
        let entries = crate::plugin_search::resolve(
            &crate::plugin_search::PluginSearchInput {
                query: query.into(),
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
        if generation.load(Ordering::Relaxed) != ticket {
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
    for location in crate::volumes::locations() {
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
mod tests {
    use super::*;

    fn search(root: &Path, query: &str, hidden: bool, ignores: &[String]) -> Vec<Entry> {
        let state = SearchState::default();
        let found = Mutex::new(Vec::new());
        state
            .run(
                "test",
                root.to_str().unwrap(),
                query,
                hidden,
                ignores,
                |batch| found.lock().unwrap().extend(batch.entries),
            )
            .unwrap();
        found.into_inner().unwrap()
    }

    #[test]
    fn finds_app_bundles_without_entering_them() {
        let t = tempfile::tempdir().unwrap();
        let bundle = t.path().join("Safari.app");
        std::fs::create_dir_all(bundle.join("Contents")).unwrap();
        std::fs::write(bundle.join("Contents").join("Needle.plist"), "x").unwrap();
        std::fs::write(t.path().join("Needle.txt"), "x").unwrap();

        let found = search(t.path(), "needle", false, &[]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Needle.txt");

        let apps = search(t.path(), "safari", false, &[]);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "Safari.app");
        assert!(apps[0].is_dir);
    }

    #[test]
    fn finds_hidden_when_requested() {
        let t = tempfile::tempdir().unwrap();
        std::fs::write(t.path().join(".Needle.txt"), "x").unwrap();
        assert_eq!(search(t.path(), "needle", false, &[]).len(), 0);
        assert_eq!(search(t.path(), "needle", true, &[]).len(), 1);
    }

    #[test]
    fn display_ignores_filter_results() {
        let t = tempfile::tempdir().unwrap();
        std::fs::create_dir(t.path().join("node_modules")).unwrap();
        std::fs::write(t.path().join("node_modules").join("Needle.js"), "x").unwrap();
        std::fs::write(t.path().join("Needle.txt"), "x").unwrap();
        let found = search(t.path(), "needle", false, &["node_modules".into()]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Needle.txt");
    }

    #[test]
    fn live_results_follow_filesystem_changes() {
        let t = tempfile::tempdir().unwrap();
        let folder = t.path().join("OldFolder");
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("Needle.txt"), "x").unwrap();
        assert_eq!(search(t.path(), "needle", false, &[]).len(), 1);
        std::fs::remove_dir_all(&folder).unwrap();
        assert_eq!(search(t.path(), "needle", false, &[]).len(), 0);
    }

    #[test]
    fn stops_at_the_result_limit() {
        let t = tempfile::tempdir().unwrap();
        for index in 0..(RESULT_LIMIT + 20) {
            std::fs::write(t.path().join(format!("Needle-{index}.txt")), "x").unwrap();
        }
        let state = SearchState::default();
        let result = Mutex::new((0usize, false));
        state
            .run(
                "test",
                t.path().to_str().unwrap(),
                "needle",
                false,
                &[],
                |batch| {
                    let mut result = result.lock().unwrap();
                    result.0 = batch.entries.len();
                    result.1 = batch.truncated;
                },
            )
            .unwrap();
        let (count, truncated) = result.into_inner().unwrap();
        assert_eq!(count, RESULT_LIMIT);
        assert!(truncated);
    }
}
