use crate::files::{entry, Entry};
use ignore::{WalkBuilder, WalkState};
use serde::Serialize;
use std::{
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};
#[derive(Default, Clone)]
pub struct SearchState(pub Arc<AtomicU64>);
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
    pub fn cancel(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
    pub fn run(
        &self,
        root: &str,
        query: &str,
        hidden: bool,
        respect_ignore: bool,
        emit: impl Fn(SearchBatch) + Send + Sync,
    ) {
        let generation = self.0.fetch_add(1, Ordering::SeqCst) + 1;
        let started = Instant::now();
        let query = query.to_lowercase();
        let buffer = Mutex::new(Vec::new());
        let count = AtomicUsize::new(0);
        let skipped = AtomicUsize::new(0);
        let threads = std::thread::available_parallelism()
            .map_or(4, |n| n.get())
            .clamp(2, 12);
        WalkBuilder::new(root)
            .hidden(!hidden)
            .ignore(respect_ignore)
            .git_ignore(respect_ignore)
            .git_global(respect_ignore)
            .git_exclude(respect_ignore)
            .parents(respect_ignore)
            .follow_links(false)
            .threads(threads)
            .build_parallel()
            .run(|| {
                Box::new(|item| {
                    if self.0.load(Ordering::Relaxed) != generation
                        || count.load(Ordering::Relaxed) >= 10000
                    {
                        return WalkState::Quit;
                    }
                    match item {
                        Ok(item)
                            if item.depth() > 0
                                && item
                                    .file_name()
                                    .to_string_lossy()
                                    .to_lowercase()
                                    .contains(&query) =>
                        {
                            if let Ok(e) = entry(item.path()) {
                                let mut buf = buffer.lock().unwrap();
                                if count.fetch_add(1, Ordering::Relaxed) < 10000 {
                                    buf.push(e);
                                }
                                if buf.len() >= 40 || count.load(Ordering::Relaxed) == 1 {
                                    emit(SearchBatch {
                                        entries: std::mem::take(&mut *buf),
                                        done: false,
                                        elapsed_ms: started.elapsed().as_millis(),
                                        truncated: false,
                                        skipped: skipped.load(Ordering::Relaxed),
                                    });
                                }
                            }
                        }
                        Err(_) => {
                            skipped.fetch_add(1, Ordering::Relaxed);
                        }
                        _ => {}
                    }
                    WalkState::Continue
                })
            });
        if self.0.load(Ordering::Relaxed) == generation {
            emit(SearchBatch {
                entries: std::mem::take(&mut *buffer.lock().unwrap()),
                done: true,
                elapsed_ms: started.elapsed().as_millis(),
                truncated: count.load(Ordering::Relaxed) >= 10000,
                skipped: skipped.load(Ordering::Relaxed),
            });
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finds_hidden_when_requested() {
        let t = tempfile::tempdir().unwrap();
        std::fs::write(t.path().join(".Needle.txt"), "x").unwrap();
        let state = SearchState::default();
        let out = Mutex::new(Vec::new());
        state.run(t.path().to_str().unwrap(), "needle", true, false, |b| {
            out.lock().unwrap().extend(b.entries)
        });
        assert_eq!(out.lock().unwrap().len(), 1);
    }
}
