use super::*;

#[derive(Deserialize)]
pub(crate) struct SearchInput {
    pane: String,
    root: String,
    query: String,
    #[serde(default = "default_search_mode")]
    mode: String,
    #[serde(default = "default_search_engine")]
    engine: String,
    #[serde(default)]
    remote_content_approved: bool,
    hidden: bool,
    ignores: Vec<String>,
}

fn default_search_mode() -> String { "name".into() }
fn default_search_engine() -> String { "auto".into() }

#[tauri::command]
pub(crate) async fn search_files(
    window: tauri::Window,
    request: SearchInput,
    on_batch: Channel<SearchBatch>,
    state: tauri::State<'_, SearchState>,
) -> Result<(), String> {
    let SearchInput {
        pane,
        root,
        query,
        mode,
        engine,
        remote_content_approved,
        hidden,
        ignores,
    } = request;
    if !matches!(mode.as_str(), "name" | "content") {
        return Err("invalid search mode".into());
    }
    if engine != "auto" && engine != "builtin" && !engine.starts_with("plugin:") {
        return Err("invalid search engine".into());
    }
    if mode == "content" && root == "everywhere://" {
        return Err("Choose a folder for content search".into());
    }
    let scope = format!("{}:{pane}", window.label());
    if root == "everywhere://" {
        let state = state.inner().clone();
        return tauri::async_runtime::spawn_blocking(move || {
            state.run_global(&scope, &query, hidden, &ignores, |b| {
                let _ = on_batch.send(b);
            })
        })
        .await
        .map_err(|error| error.to_string())?;
    }
    // One scoped ticket owns cancellation and stale-result rejection for every
    // storage adapter; the root never selects a source-specific search path.
    let ticket = state.begin(&scope);
    let token = ticket.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        if token.cancelled() {
            return Ok(());
        }
        let started = std::time::Instant::now();
        let resource = crate::core::fs::trusted(&root, "core:search")?;
        let search_request = crate::plugins::resolvers::search::PluginSearchInput {
            query: query.clone(), mode: mode.clone(), engine: engine.clone(), root: root.clone(),
            hidden, ignores: ignores.clone(), limit: Some(500),
        };
        if mode == "content" && std::path::Path::new(&root).is_dir()
            && engine != "builtin"
            && crate::plugins::resolvers::search::has_plugin(&search_request) {
            let entries = crate::plugins::resolvers::search::resolve(
                &search_request,
                || {
                    let found = std::sync::Mutex::new(Vec::new());
                    crate::core::search::search_local_stream(
                        std::path::Path::new(&root), &query, "content", hidden,
                        &ignores, &|| token.cancelled(),
                        |batch| found.lock().unwrap().extend(batch.entries),
                    )?;
                    Ok(serde_json::json!({"entries":found.into_inner().unwrap()}))
                },
                || token.cancelled(),
            );
            if !token.cancelled() {
                let _ = on_batch.send(SearchBatch { entries, done: true,
                    elapsed_ms: started.elapsed().as_millis(), truncated: false, skipped: 0 });
            }
            return Ok(());
        }
        if mode == "content" || (std::path::Path::new(&root).is_dir()
            && !root.contains("://")
            && !crate::plugins::resolvers::search::has_plugin(&search_request)) {
            if mode == "content" && crate::plugins::mounts::handles(&root) {
                if !remote_content_approved {
                    return Err("Remote content fallback requires confirmation".into());
                }
                resource.require(crate::plugins::provider_registry::FsOperation::List, false)?;
                return crate::core::search::search_vfs_content_stream(
                    &root, &query, hidden, &ignores, &|| token.cancelled(),
                    |batch| { let _ = on_batch.send(batch); },
                );
            }
            if root.contains("://") || !std::path::Path::new(&root).is_dir() {
                return Err("Content search is unavailable for this resource".into());
            }
            resource.require(crate::plugins::provider_registry::FsOperation::List, false)?;
            let local_root = std::path::Path::new(&root);
            return crate::core::search::search_local_stream(
                local_root, &query, &mode, hidden, &ignores, &|| token.cancelled(),
                |batch| { let _ = on_batch.send(batch); },
            );
        }
        let (entries, truncated, skipped) =
            resource.search(&query, hidden, &ignores, 500, &|| token.cancelled())?;
        if token.cancelled() {
            return Ok(());
        }
        let entries = crate::plugins::resolvers::search::resolve(
            &search_request,
            || Ok(serde_json::json!({"entries":entries})),
            || token.cancelled(),
        );
        let _ = on_batch.send(SearchBatch {
            entries,
            done: true,
            elapsed_ms: started.elapsed().as_millis(),
            truncated,
            skipped,
        });
        Ok(())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub(crate) fn cancel_search(
    window: tauri::Window,
    pane: String,
    state: tauri::State<'_, SearchState>,
) {
    state.cancel(&format!("{}:{pane}", window.label()));
}
