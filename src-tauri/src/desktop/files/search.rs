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
    if remote::is_remote(&root) {
        let started = std::time::Instant::now();
        let mut pending = vec![root.clone()];
        let mut entries = Vec::new();
        let mut visited = 0;
        while let Some(path) = pending.pop() {
            visited += 1;
            if visited > 1000 || entries.len() >= 500 {
                break;
            }
            if let Some(listing) = remote::list_listing(&path, hidden, &ignores).await? {
                for entry in listing.entries {
                    if entry.is_dir {
                        pending.push(entry.path.clone());
                    }
                    if entry.name.to_lowercase().contains(&query.to_lowercase()) {
                        entries.push(entry);
                    }
                    if entries.len() >= 500 {
                        break;
                    }
                }
            }
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
            || false,
        );
        let _ = on_batch.send(SearchBatch {
            entries,
            done: true,
            elapsed_ms: started.elapsed().as_millis(),
            truncated: !pending.is_empty() || visited > 1000,
            skipped: 0,
        });
        return Ok(());
    }
    if crate::plugins::vfs::handles(&root) {
        return tauri::async_runtime::spawn_blocking(move || {
            let started = std::time::Instant::now();
            let (entries, truncated) =
                crate::plugins::vfs::search(&root, &query, hidden, &ignores, 500)?;
            let entries = crate::plugins::resolvers::search::resolve(
                &crate::plugins::resolvers::search::PluginSearchInput {
                    query: query.clone(),
                    root: root.clone(),
                    hidden,
                    ignores: ignores.clone(),
                    limit: Some(500),
                },
                || Ok(serde_json::json!({"entries":entries})),
                || false,
            );
            let _ = on_batch.send(SearchBatch {
                entries,
                done: true,
                elapsed_ms: started.elapsed().as_millis(),
                truncated,
                skipped: 0,
            });
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())?;
    }
    if let Some(route) = crate::usbfs::route(&root) {
        return tauri::async_runtime::spawn_blocking(move || {
            let started = std::time::Instant::now();
            let entries = crate::usbfs::search(&route, &query, 500)?;
            let entries = crate::plugins::resolvers::search::resolve(
                &crate::plugins::resolvers::search::PluginSearchInput {
                    query: query.clone(),
                    root: root.clone(),
                    hidden,
                    ignores: ignores.clone(),
                    limit: Some(500),
                },
                || Ok(serde_json::json!({"entries":entries})),
                || false,
            );
            let _ = on_batch.send(SearchBatch {
                entries,
                done: true,
                elapsed_ms: started.elapsed().as_millis(),
                truncated: false,
                skipped: 0,
            });
            Ok(())
        })
        .await
        .map_err(|error| error.to_string())?;
    }
    let state = state.inner().clone();
    let scope = format!("{}:{pane}", window.label());
    tauri::async_runtime::spawn_blocking(move || {
        if root == "everywhere://" {
            state.run_global(&scope, &query, hidden, &ignores, |b| {
                let _ = on_batch.send(b);
            })
        } else {
            state.run(&scope, &root, &query, hidden, &ignores, |b| {
                let _ = on_batch.send(b);
            })
        }
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(())
}

#[tauri::command]
pub(crate) fn cancel_search(
    window: tauri::Window,
    pane: String,
    state: tauri::State<'_, SearchState>,
) {
    state.cancel(&format!("{}:{pane}", window.label()));
}
