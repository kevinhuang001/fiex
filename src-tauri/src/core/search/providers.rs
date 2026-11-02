//! Search-provider selection, VFS snapshot reconciliation, and accessible roots.
use super::{adapter_content, BatchSink, SearchBatch, SearchEngine, SearchMode, SearchRequest};
use crate::core::files::Entry;
use std::{path::PathBuf, sync::Mutex};

pub(super) fn search_root(
    root: &str,
    request: &SearchRequest,
    cancelled: &(dyn Fn() -> bool + Sync),
    sink: &mut BatchSink<'_>,
) -> Result<(), String> {
    // Core-owned virtual roots are not host paths: filter their root listing
    // instead of asking a filesystem SearchProvider about a URI.
    if virtual_search_root(root)
        || (request.mode == SearchMode::Name
            && crate::core::fs::provider_registry::is_plugin_mount(root))
    {
        return search_cached_vfs(root, request, cancelled, sink);
    }
    let local = crate::core::fs::provider_registry::resource_access(root)
        .is_ok_and(|access| access == crate::core::fs::provider_registry::ResourceAccess::HostPath);
    if !local {
        return search_adapter(root, request, cancelled, sink);
    }
    let input = crate::plugins::providers::search::PluginSearchInput {
        query: request.query.clone(),
        root: root.to_string(),
        hidden: request.hidden,
        mode: request.mode,
        ignores: request.ignores.clone(),
        limit: Some(sink.limit),
    };
    let engines = crate::plugins::providers::search::engines(&input);
    let builtin = engines
        .iter()
        .find(|engine| engine.is_builtin() && engine.supports(request.mode))
        .cloned();
    let selected = match &request.engine {
        SearchEngine::Builtin => builtin.clone(),
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
        sink.engine = Some(engine.resolver.name.clone());
        let mut emitted = false;
        let result = crate::plugins::providers::search::run_stream(
            &engine,
            &input,
            cancelled,
            &mut |entry| {
                emitted = true;
                sink.push(entry);
            },
        );
        match result {
            Ok(truncated) => {
                sink.truncated |= truncated;
                return Ok(());
            }
            Err(error) => {
                if cancelled() {
                    return Ok(());
                }
                if matches!(request.engine, SearchEngine::Auto) {
                    sink.warning = Some(format!("{}: {error}", engine.resolver.name));
                    if emitted {
                        // Partial pages were already streamed. Do not append
                        // a different engine's results into the same session.
                        sink.incomplete = true;
                        return Ok(());
                    }
                } else {
                    return Err(error);
                }
            }
        }
    }
    if let Some(engine) = builtin {
        sink.engine = Some("builtin".into());
        let mut emitted = false;
        match crate::plugins::providers::search::run_stream(
            &engine,
            &input,
            cancelled,
            &mut |entry| {
                emitted = true;
                sink.push(entry);
            },
        ) {
            Ok(truncated) => sink.truncated |= truncated,
            Err(_error) if cancelled() => return Ok(()),
            Err(error) if emitted => {
                sink.incomplete = true;
                sink.warning = Some(error);
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

/// A virtual search root owns a cached subtree (a root or one of its
/// subdirectories). Only cached directories are searched; uncached directories
/// are never enumerated.
pub(super) fn virtual_search_root(root: &str) -> bool {
    if crate::core::fs::provider_registry::is_plugin_mount(root) {
        return false;
    }
    crate::core::volumes::native_vfs::is_tree_path(root)
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
    let upstream = &mut *sink.emit;
    let mut emit = |mut batch: SearchBatch| {
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
    let mut fresh = BatchSink::new(&mut emit, sink.started, sink.limit);
    let result = search_adapter(root, request, cancelled, &mut fresh);
    fresh.flush(false);
    if cancelled() {
        return Ok(());
    }
    if let Err(error) = result {
        drop(fresh);
        drop(emit);
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
    drop(emit);
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
    sink.engine = Some("builtin".into());
    if request.mode == SearchMode::Content {
        return adapter_content::run(root, request, cancelled, sink);
    }
    let resource = crate::core::fs::trusted(root, "core:search")?;
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

/// Only filesystem roots readable with the application's own user token are
/// scanned; protected paths naturally fail and are recorded as skipped.
pub fn accessible_roots() -> Vec<String> {
    let home = dirs::home_dir().filter(|path| path.is_dir());
    let mut roots = Vec::new();
    for location in crate::core::volumes::locations() {
        if location.kind != "disk"
            || !location.mounted
            || !crate::core::fs::provider_registry::resource_access(&location.path).is_ok_and(
                |access| access == crate::core::fs::provider_registry::ResourceAccess::HostPath,
            )
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
