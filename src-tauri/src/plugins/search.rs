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

/// Local traversal and plugin sources use the same event/result policy. The
/// caller's cancellation generation is checked before each source and collection.
pub fn resolve(
    request: &PluginSearchInput,
    builtin: impl FnMut() -> Result<serde_json::Value, String>,
    cancelled: impl Fn() -> bool,
) -> Vec<Entry> {
    use crate::plugin_events::{EventRouter, Handler, Provider};
    use serde_json::{json, Value};
    let limit = request.limit.unwrap_or(500).clamp(1, 500);
    let resource = crate::plugin_resource_broker::Resource::resolve(&request.root).ok();
    let metadata = resource
        .as_ref()
        .map(|r| vec![r.metadata.clone()])
        .unwrap_or_default();
    let providers = EventRouter::providers(
        "search.query",
        &metadata,
        &request.root,
        vec![Provider::builtin("core.search", 1000)],
    );
    let request_json = json!({"target":resource.as_ref().map(|r|r.project(true)),"selection":[],"options":{
        "query":request.query,"root":request.root,"hidden":request.hidden,"ignores":request.ignores,"limit":limit}});
    let mut builtin = builtin;
    let outcomes = EventRouter::resolve(
        "search.query",
        providers,
        |_, _| {
            if cancelled() {
                Err("search cancelled".into())
            } else {
                builtin()
            }
        },
        |plugin| {
            if cancelled() {
                return Err("search cancelled".into());
            }
            crate::plugin_events::invoke_directory(
                &plugin.id,
                "search.query",
                request_json.clone(),
                &request.root,
                &[],
            )
        },
        |provider, response| {
            let items = response
                .get("entries")
                .and_then(Value::as_array)
                .filter(|items| items.len() <= 500)
                .ok_or("invalid search entries")?;
            let mut entries = Vec::new();
            for item in items.iter().take(limit) {
                let entry = match &provider.handler {
                    Handler::Builtin(_) => {
                        serde_json::from_value::<Entry>(item.clone()).map_err(|e| e.to_string())?
                    }
                    Handler::Plugin(_) => crate::plugin_vfs::entry_from(item.clone())?,
                };
                // A provider cannot expand the requested search root.
                if !crate::mounts::handles(&request.root) {
                    let root = std::path::Path::new(&request.root);
                    if !std::path::Path::new(&entry.path).starts_with(root) {
                        return Err("search result outside requested root".into());
                    }
                } else if !entry
                    .path
                    .starts_with(&format!("{}/", request.root.trim_end_matches('/')))
                    && entry.path != request.root
                {
                    return Err("search result outside requested mount".into());
                }
                entries.push(entry);
            }
            Ok(entries)
        },
        |_| true,
    );
    let mut entries = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (_, result) in outcomes {
        if cancelled() {
            return Vec::new();
        }
        if let Ok(values) = result {
            for entry in values {
                let key = if cfg!(windows) {
                    entry.path.replace('\\', "/").to_lowercase()
                } else {
                    entry.path.clone()
                };
                if seen.insert(key) {
                    entries.push(entry);
                }
            }
        }
    }
    entries.truncate(limit);
    entries
}

pub fn query(request: &PluginSearchInput) -> Vec<Entry> {
    resolve(request, || Ok(serde_json::json!({"entries":[]})), || false)
}
