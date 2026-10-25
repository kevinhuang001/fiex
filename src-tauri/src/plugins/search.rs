//! Aggregation of plugin-provided search results. Core owns the query
//! lifecycle, cancellation, deduplication and result UI; providers only return
//! structured entries.
use crate::files::Entry;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSearchInput {
    pub query: String,
    pub root: String,
    pub hidden: bool,
    #[serde(default)]
    pub ignores: Vec<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

pub fn query(request: &PluginSearchInput) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let limit = request.limit.unwrap_or(200).clamp(1, 500);
    for plugin in crate::plugin_host::catalog() {
        if !plugin.enabled || !plugin.backend_available {
            continue;
        }
        if !plugin
            .extensions
            .get("search")
            .is_some_and(serde_json::Value::is_object)
        {
            continue;
        }
        let response = match crate::plugin_host::global().invoke(
            &plugin.id,
            "search.query",
            serde_json::json!({
                "query": request.query,
                "root": request.root,
                "hidden": request.hidden,
                "ignores": request.ignores,
                "limit": limit
            }),
        ) {
            Ok(response) => response,
            // One broken provider must not suppress local or other
            // providers' results.
            Err(error) => {
                eprintln!("plugin search {} failed: {error}", plugin.id);
                continue;
            }
        };
        let Some(items) = response
            .get("entries")
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };
        for item in items.iter().take(limit) {
            let entry = match crate::plugin_vfs::entry_from(item.clone()) {
                Ok(entry) => entry,
                Err(error) => {
                    eprintln!(
                        "plugin search {} returned an invalid entry: {error}",
                        plugin.id
                    );
                    continue;
                }
            };
            if seen.insert(entry.path.clone()) {
                entries.push(entry);
            }
        }
    }
    entries.truncate(limit * 4);
    entries
}
