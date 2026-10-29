use super::*;

#[derive(Deserialize)]
pub(crate) struct SearchInput {
    pane: String,
    root: String,
    query: String,
    hidden: bool,
    ignores: Vec<String>,
}

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
        hidden,
        ignores,
    } = request;
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
        let (entries, truncated, skipped) =
            resource.search(&query, hidden, &ignores, 500, &|| token.cancelled())?;
        if token.cancelled() {
            return Ok(());
        }
        let entries = crate::plugins::resolvers::search::resolve(
            &crate::plugins::resolvers::search::PluginSearchInput {
                query: query.clone(),
                root: root.clone(),
                hidden,
                ignores: ignores.clone(),
                limit: Some(500),
            },
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
