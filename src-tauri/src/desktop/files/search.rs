use super::*;

#[derive(Deserialize)]
pub(crate) struct SearchInput {
    pane: String,
    root: String,
    query: String,
    hidden: bool,
    ignores: Vec<String>,
    #[serde(default)]
    mode: SearchMode,
    #[serde(default)]
    engine: SearchEngine,
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
        mode,
        engine,
    } = request;
    let scope = format!("{}:{pane}", window.label());
    let search = SearchRequest {
        root,
        query,
        mode,
        engine,
        hidden,
        ignores,
        limit: RESULT_LIMIT,
    };
    // One scoped generation owns cancellation and stale-result rejection for
    // every storage adapter; the root never selects a source-specific path.
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state.stream(&scope, &search, |batch| {
            let _ = on_batch.send(batch);
        })
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
