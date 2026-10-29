//! Aggregation of plugin-provided search results. Core owns the query
//! lifecycle, cancellation, deduplication and result UI; providers only return
//! structured entries.
use crate::core::files::Entry;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSearchInput {
    pub query: String,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_engine")]
    pub engine: String,
    pub root: String,
    pub hidden: bool,
    #[serde(default)]
    pub ignores: Vec<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

fn default_mode() -> String { "name".into() }
fn default_engine() -> String { "auto".into() }

/// Search policy: select one available engine, validate and confine its results.
pub fn resolve(
    request: &PluginSearchInput,
    mut builtin: impl FnMut() -> Result<serde_json::Value, String>,
    cancelled: impl Fn() -> bool,
) -> Vec<Entry> {
    use crate::plugins::events::{EventRouter, Handler};
    use serde_json::json;
    let limit = request.limit.unwrap_or(500).clamp(1, 500);
    let ignores = crate::core::ignores::IgnoreSet::new(&request.ignores);
    let resources =
        crate::plugins::resource::resource_context::ResourceSnapshot::resolve(&request.root)
            .map(|r| vec![r])
            .unwrap_or_default();
    let providers = candidates(request, &resources);
    EventRouter::resolve(
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
            crate::plugins::events::invoke_resources(
                plugin,
                "search.query",
                json!({"query":request.query,"mode":request.mode,"root":resources.first().map(|r| &r.reference),"hidden":request.hidden,"ignores":request.ignores,"limit":limit}),
                &resources,
                Some(&request.root),
            )
        },
        |provider, response| {
            let items = response["entries"]
                .as_array()
                .ok_or("invalid search entries")?;
            let mut values = Vec::new();
            for value in items.iter().take(limit) {
                let mut entry = match provider.handler {
                    Handler::Builtin(_) => {
                        serde_json::from_value::<Entry>(value.clone()).map_err(|e| e.to_string())?
                    }
                    Handler::Plugin(_) => crate::plugins::vfs::entry_from(value.clone())?,
                };
                if crate::plugins::mounts::handles(&request.root) {
                    let root = crate::plugins::mounts::parse(&request.root)?;
                    let path = crate::plugins::mounts::parse(&entry.path)?;
                    if root.mount_id != path.mount_id
                        || !std::path::Path::new(&path.relative).starts_with(&root.relative)
                    {
                        return Err("search result outside requested mount".into());
                    }
                } else {
                    let root = dunce::canonicalize(&request.root).map_err(|e| e.to_string())?;
                    // Providers receive the root as an opaque ref. A local
                    // provider may therefore return a path relative to that
                    // root; resolve it here and never trust a bare relative
                    // path as a host location.
                    let candidate = std::path::Path::new(&entry.path);
                    let candidate = if candidate.is_absolute() {
                        candidate.to_path_buf()
                    } else {
                        root.join(candidate)
                    };
                    let path =
                        crate::plugins::resource::fs_access::canonical_selection(&candidate)?;
                    if !path.starts_with(&root) {
                        return Err("search result outside requested root".into());
                    }
                    entry.path = path.to_string_lossy().into_owned();
                }
                if !request.hidden && entry.name.starts_with('.') {
                    continue;
                }
                let relative = entry
                    .path
                    .strip_prefix(&request.root)
                    .unwrap_or(&entry.name)
                    .trim_start_matches(['/', '\\']);
                if ignores.is_ignored(std::path::Path::new(relative)) {
                    continue;
                }
                values.push(entry);
            }
            Ok(values)
        },
        |_| true,
        |outcomes| {
            let mut seen = std::collections::HashSet::new();
            let mut entries = Vec::new();
            for (_, result) in outcomes.into_iter().filter(|(_, result)| result.is_ok()).take(1) {
                if cancelled() {
                    return Vec::new();
                }
                if let Ok(values) = result {
                    for entry in values {
                        let remote = crate::plugins::mounts::handles(&entry.path);
                        let key = if remote {
                            let uri = crate::plugins::mounts::parse(&entry.path)
                                .expect("validated search URI");
                            format!("{}/{}", uri.mount_id, uri.relative)
                        } else {
                            dunce::canonicalize(&entry.path)
                                .unwrap_or_else(|_| entry.path.clone().into())
                                .to_string_lossy()
                                .into_owned()
                        };
                        let key = if cfg!(windows) && !remote {
                            key.to_lowercase()
                        } else {
                            key
                        };
                        if seen.insert(key) {
                            entries.push(entry);
                        }
                    }
                }
            }
            entries.truncate(limit);
            entries
        },
    )
}
fn candidates(request: &PluginSearchInput, resources: &[crate::plugins::resource::resource_context::ResourceSnapshot]) -> Vec<crate::plugins::events::Resolver> {
    use crate::plugins::events::{EventRouter, Handler, Resolver};
    let metadata = crate::plugins::events::resource_metadata(resources);
    let mut providers = EventRouter::providers(
        "search.query", &metadata, &request.root,
        vec![Resolver::builtin("core.search", -1000)],
    );
    providers.retain(|provider| match &provider.handler {
        Handler::Builtin(_) => true,
        Handler::Plugin(plugin) => {
            if crate::plugins::mounts::handles(&request.root)
                || crate::remote::is_remote(&request.root) { return false; }
            let search = &plugin.extensions["search"];
            let scope = search["scope"].as_str().unwrap_or("local");
            (scope == "local" || scope == "global")
                && search["modes"].as_array()
                    .map(|modes| modes.iter().any(|mode| mode == &request.mode))
                    .unwrap_or(request.mode == "name")
        }
    });
    if request.engine == "builtin" {
        providers.retain(|provider| matches!(&provider.handler, Handler::Builtin(_)));
    } else if request.engine.starts_with("plugin:") {
        providers.retain(|provider| provider.id == request.engine
            || matches!(&provider.handler, Handler::Builtin(_)));
    }
    providers
}

pub fn has_plugin(request: &PluginSearchInput) -> bool {
    let resources = crate::plugins::resource::resource_context::ResourceSnapshot::resolve(&request.root)
        .map(|resource| vec![resource]).unwrap_or_default();
    candidates(request, &resources).iter().any(|provider|
        matches!(&provider.handler, crate::plugins::events::Handler::Plugin(_)))
}
pub fn query(request: &PluginSearchInput) -> Vec<Entry> {
    resolve(request, || Ok(serde_json::json!({"entries":[]})), || false)
}
