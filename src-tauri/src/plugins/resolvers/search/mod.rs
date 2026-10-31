//! SearchProvider routing for Local FS queries.
//!
//! Core owns the query lifecycle and chooses **one** engine per query: the
//! user's explicit engine, the first available provider that declares the
//! requested mode, or the builtin walker as the last candidate. Every result
//! is validated against the authorized root before it leaves this module.
use crate::core::files::Entry;
use crate::plugin_sdk::provider_contracts::{ProviderEndpoint, ProviderRequest};
use crate::plugins::events::EventRouter;
mod endpoint;
use crate::core::search::SearchMode;
use crate::plugins::events::{Handler, Resolver};
use serde::Deserialize;
use serde_json::{json, Value};

pub const BUILTIN_SEARCH: &str = "core.search";
/// The builtin walker is the final candidate in Auto mode; a plugin that
/// declares a higher priority becomes the default engine instead.
const BUILTIN_PRIORITY: i64 = 10;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSearchInput {
    pub query: String,
    pub root: String,
    pub hidden: bool,
    #[serde(default)]
    pub mode: SearchMode,
    #[serde(default)]
    pub ignores: Vec<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

impl PluginSearchInput {
    pub fn limit(&self) -> usize {
        self.limit.unwrap_or(500).clamp(1, 500)
    }
}

/// A local-search candidate plus the modes it really declares.
#[derive(Clone)]
pub struct SearchEngineInfo {
    pub resolver: Resolver,
    pub modes: Vec<SearchMode>,
}

impl SearchEngineInfo {
    pub fn supports(&self, mode: SearchMode) -> bool {
        self.modes.contains(&mode)
    }
    pub fn is_builtin(&self) -> bool {
        matches!(self.resolver.handler, Handler::Builtin(_))
    }
    pub fn json(&self) -> Value {
        let mut value = self.resolver.json();
        value["builtin"] = Value::Bool(self.is_builtin());
        value["modes"] = json!(self
            .modes
            .iter()
            .map(|mode| mode.as_str())
            .collect::<Vec<_>>());
        value
    }
}

/// Explicitly declared modes for one provider. Missing or invalid mode
/// declarations never silently grant search capabilities.
fn modes_of(provider: &Resolver) -> Vec<SearchMode> {
    let Handler::Plugin(plugin) = &provider.handler else {
        return vec![SearchMode::Name, SearchMode::Content];
    };
    let Some(contribution) = plugin.extensions.get("search") else {
        return Vec::new();
    };
    if !contribution.is_object() && contribution.as_bool() != Some(true) {
        return Vec::new();
    }
    // Eligibility for Local FS is handled by the standard trusted
    // storage match. No obsolete "scope" or global-disks aliases.
    let Some(modes) = contribution.get("modes").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for mode in modes.iter().filter_map(Value::as_str) {
        match mode {
            "name" if !out.contains(&SearchMode::Name) => out.push(SearchMode::Name),
            "content" if !out.contains(&SearchMode::Content) => out.push(SearchMode::Content),
            _ => {}
        }
    }
    out
}

fn input(request: &PluginSearchInput) -> Value {
    let resources =
        crate::plugins::resource::resource_context::ResourceSnapshot::resolve(&request.root)
            .map(|resource| vec![resource])
            .unwrap_or_default();
    json!({
        "query": request.query,
        "root": resources.first().map(|resource| &resource.reference),
        "hidden": request.hidden,
        "mode": request.mode.as_str(),
        "ignores": request.ignores,
        "limit": request.limit(),
    })
}

fn resources(
    request: &PluginSearchInput,
) -> Vec<crate::plugins::resource::resource_context::ResourceSnapshot> {
    crate::plugins::resource::resource_context::ResourceSnapshot::resolve(&request.root)
        .map(|resource| vec![resource])
        .unwrap_or_default()
}

/// Local-search candidates in effective order (user preference applied), with
/// the builtin walker last by default priority.
pub fn engines(request: &PluginSearchInput) -> Vec<SearchEngineInfo> {
    let resources = resources(request);
    let providers = crate::plugins::provider_registry::candidates(
        "search.query",
        &crate::plugins::events::resource_metadata(&resources),
        &request.root,
        vec![Resolver::builtin(BUILTIN_SEARCH, BUILTIN_PRIORITY)],
    );
    providers
        .into_iter()
        .map(|resolver| SearchEngineInfo {
            modes: modes_of(&resolver),
            resolver,
        })
        .filter(|engine| !engine.modes.is_empty())
        .collect()
}

/// Invoke exactly one plugin engine and return its validated entries.
pub fn run(
    engine: &SearchEngineInfo,
    request: &PluginSearchInput,
    cancelled: impl Fn() -> bool,
) -> Result<(Vec<Entry>, bool), String> {
    if engine.is_builtin() {
        return Err("builtin search engine runs in Core".into());
    }
    if !engine.supports(request.mode) {
        return Err(format!(
            "{} 不支持{}搜索",
            engine.resolver.name,
            match request.mode {
                SearchMode::Name => "文件名",
                SearchMode::Content => "文件内容",
            }
        ));
    }
    if cancelled() {
        return Err("search cancelled".into());
    }
    let resources = resources(request);
    let call = ProviderRequest {
        method: "search.query".into(),
        params: input(request),
    };
    EventRouter::resolve(
        "search.query",
        vec![engine.resolver.clone()],
        &call,
        |resolver| {
            let Handler::Plugin(plugin) = &resolver.handler else {
                return Err("native search runs through the streaming walker".into());
            };
            Ok(Box::new(endpoint::PluginSearchEndpoint {
                plugin: (**plugin).clone(),
                resources: &resources,
                directory: &request.root,
            }) as Box<dyn ProviderEndpoint + '_>)
        },
        |resolver, response| {
            let truncated = response["truncated"].as_bool().unwrap_or(false);
            Ok((validated(resolver, response, request)?, truncated))
        },
        |_| true,
        |results| {
            results
                .into_iter()
                .next()
                .ok_or_else(|| "search provider unavailable".to_string())?
                .1
        },
    )
}

/// Shared result validation: ownership, root confinement, hidden/ignore rules,
/// and bounded size. A provider can never widen the authorized search scope.
fn validated(
    provider: &Resolver,
    response: Value,
    request: &PluginSearchInput,
) -> Result<Vec<Entry>, String> {
    let limit = request.limit();
    let ignores = crate::core::ignores::IgnoreSet::new(&request.ignores);
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
            // Providers receive the root as an opaque ref. A local provider may
            // therefore return a path relative to that root; resolve it here and
            // never trust a bare relative path as a host location.
            let candidate = std::path::Path::new(&entry.path);
            let candidate = if candidate.is_absolute() {
                candidate.to_path_buf()
            } else {
                root.join(candidate)
            };
            let path = crate::plugins::resource::fs_access::canonical_selection(&candidate)?;
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
}
