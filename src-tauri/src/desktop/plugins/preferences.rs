//! Shared provider preference commands and the search engine list.
//!
//! Every priority-aware feature uses one store; the settings UI edits it
//! through these commands instead of keeping a per-feature list in the webview.
#[tauri::command]
pub(crate) fn provider_preferences() -> serde_json::Value {
    crate::plugins::preferences::snapshot()
}

#[tauri::command]
pub(crate) fn set_provider_preference(
    feature: String,
    order: Vec<String>,
    disabled: Vec<String>,
) -> Result<serde_json::Value, String> {
    crate::plugins::preferences::set(&feature, order, disabled)?;
    Ok(crate::plugins::preferences::snapshot())
}

#[tauri::command]
pub(crate) fn reset_provider_preference(feature: String) -> Result<serde_json::Value, String> {
    crate::plugins::preferences::reset(&feature)?;
    Ok(crate::plugins::preferences::snapshot())
}

/// Local search engines for the engine picker: providers that declare the
/// requested mode, followed by the builtin fallback. VFS roots never list a
/// local provider, matching the routing rules in Core.
#[tauri::command]
pub(crate) fn search_providers(root: String, mode: Option<String>) -> Vec<serde_json::Value> {
    let request = crate::plugins::resolvers::search::PluginSearchInput {
        query: String::new(),
        root,
        hidden: false,
        mode: match mode.as_deref() {
            Some("content") => crate::core::search::SearchMode::Content,
            _ => crate::core::search::SearchMode::Name,
        },
        ignores: Vec::new(),
        limit: None,
    };
    crate::plugins::resolvers::search::engines(&request)
        .into_iter()
        .map(|engine| engine.json())
        .collect()
}
