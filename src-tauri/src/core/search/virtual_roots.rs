use super::{BatchSink, SearchBatch, SearchMode, SearchRequest};
use crate::core::files::Entry;
use std::sync::Mutex;

/// A virtual search root owns a cached subtree (a root or one of its
/// subdirectories). Only cached directories are searched; uncached directories
/// are never enumerated.
pub(super) fn virtual_search_root(root: &str) -> bool {
    if crate::core::fs::provider_registry::is_plugin_mount(root) {
        return false;
    }
    crate::core::volumes::native_vfs::is_tree_path(root)
}

pub(super) fn search_cached_vfs(
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
    let (result, entries, same, truncated, skipped, scanned) = {
        let live: Mutex<Vec<Entry>> = Mutex::new(Vec::new());
        let upstream = &mut *sink.emit;
        let mut emit = |mut batch: SearchBatch| {
            {
                let mut live = live.lock().unwrap_or_else(|e| e.into_inner());
                for entry in &batch.entries {
                    if let Some(previous) =
                        live.iter_mut().find(|previous| previous.path == entry.path)
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
        let mut fresh = BatchSink::new(&mut emit, sink.started, sink.limit);
        let result = super::providers::search_adapter(root, request, cancelled, &mut fresh);
        fresh.flush(false);
        let entries = if result.is_ok() {
            live.lock().unwrap_or_else(|e| e.into_inner()).clone()
        } else {
            Vec::new()
        };
        let same = result.is_ok() && same_entry_snapshot(&cached, &entries);
        (
            result,
            entries,
            same,
            fresh.truncated,
            fresh.skipped,
            fresh.scanned,
        )
    };
    if cancelled() {
        return Ok(());
    }
    if let Err(error) = result {
        sink.warning = Some(error);
        sink.incomplete = true;
        sink.emit_snapshot(Vec::new(), true, false, false, 0, 0);
        return Ok(());
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, name: &str) -> Entry {
        Entry {
            path: path.into(),
            name: name.into(),
            ..Entry::default()
        }
    }

    #[test]
    fn cached_snapshot_comparison_ignores_order() {
        let first = entry("/docs/a.txt", "a.txt");
        let second = entry("/docs/b.txt", "b.txt");

        assert!(same_entry_snapshot(
            &[first.clone(), second.clone()],
            &[second, first],
        ));
    }

    #[test]
    fn cached_snapshot_comparison_detects_metadata_changes() {
        let original = entry("/docs/a.txt", "a.txt");
        let updated = Entry {
            size: 12,
            ..original.clone()
        };

        assert!(!same_entry_snapshot(&[original], &[updated]));
    }
}
