//! SearchProvider routing for Local FS queries.
//!
//! Core owns the query lifecycle and chooses **one** engine per query: the
//! user's explicit engine, the first available provider that declares the
//! requested mode, or the builtin walker as the last candidate. Every result
//! is validated against the authorized root before it leaves this module.
use crate::core::files::Entry;
mod native;
mod proxy;
mod stream;
use crate::core::search::SearchMode;
use crate::plugins::events::Resolver;
use fiex_provider_contracts::{SearchEntry, SearchProvider, SearchTag};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
pub use stream::{run, run_stream};

pub(crate) fn forget_plugin(plugin_id: &str) {
    proxy::forget_plugin(plugin_id);
}

pub const BUILTIN_SEARCH: &str = "core.search";
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
    pub implementation: Arc<dyn SearchProvider>,
}

impl SearchEngineInfo {
    pub fn supports(&self, mode: SearchMode) -> bool {
        self.modes.contains(&mode)
    }
    pub fn is_builtin(&self) -> bool {
        self.resolver.plugin.is_none()
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
    let Some(plugin) = provider.plugin.as_deref() else {
        return native::modes(&provider.id);
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

fn input(
    request: &PluginSearchInput,
    resources: &[crate::plugins::resource::resource_context::ResourceSnapshot],
) -> Value {
    json!({
        "query": request.query,
        "root": resources.first().map(|resource| &resource.reference),
        "hidden": request.hidden,
        "mode": request.mode.as_str(),
        "ignores": request.ignores,
        "limit": request.limit(),
    })
}

pub(crate) fn search_entry(entry: Entry) -> SearchEntry {
    SearchEntry {
        path: entry.path,
        name: entry.name,
        is_dir: entry.is_dir,
        is_symlink: entry.is_symlink,
        size: entry.size,
        modified: entry.modified,
        created: entry.created,
        extension: entry.extension,
        mime: entry.mime,
        link_target: entry.link_target,
        allowed_ops: entry.allowed_ops,
        tags: entry
            .tags
            .into_iter()
            .map(|tag| SearchTag {
                name: tag.name,
                color: tag.color,
            })
            .collect(),
        has_cache: entry.has_cache,
        pinned: entry.pinned,
        uploaded: entry.uploaded,
        sync: entry.sync,
    }
}

fn core_entry(entry: SearchEntry) -> Entry {
    Entry {
        path: entry.path,
        name: entry.name,
        is_dir: entry.is_dir,
        is_symlink: entry.is_symlink,
        size: entry.size,
        modified: entry.modified,
        created: entry.created,
        extension: entry.extension,
        mime: entry.mime,
        link_target: entry.link_target,
        allowed_ops: entry.allowed_ops,
        tags: entry
            .tags
            .into_iter()
            .map(|tag| crate::core::tags::Tag {
                name: tag.name,
                color: tag.color,
            })
            .collect(),
        has_cache: entry.has_cache,
        pinned: entry.pinned,
        uploaded: entry.uploaded,
        storage_projection: Default::default(),
        sync: entry.sync,
    }
}

fn resources(
    request: &PluginSearchInput,
) -> Vec<crate::plugins::resource::resource_context::ResourceSnapshot> {
    crate::plugins::resource::resource_context::resolve(&request.root)
        .map(|resource| vec![resource])
        .unwrap_or_default()
}

/// Local-search candidates in effective order (user preference applied), with
/// the builtin walker last by default priority.
pub fn engines(request: &PluginSearchInput) -> Vec<SearchEngineInfo> {
    let resources = resources(request);
    let targets = crate::plugins::events::matching::match_resources(&resources);
    let providers = crate::plugins::provider_registry::candidates(
        "search.query",
        &targets,
        &request.root,
        native::candidates(&targets),
    );
    providers
        .into_iter()
        .filter_map(|resolver| {
            let modes = modes_of(&resolver);
            if modes.is_empty() {
                return None;
            }
            let implementation: Arc<dyn SearchProvider> = match resolver.plugin.as_deref() {
                None => native::provider_arc(&resolver.id)?,
                Some(plugin) => Arc::new(
                    proxy::ScopedSearchProxy::new(
                        plugin.clone(),
                        &resources,
                        &request.root,
                        resolver.id.clone(),
                    )
                    .ok()?,
                ),
            };
            Some(SearchEngineInfo {
                resolver,
                modes,
                implementation,
            })
        })
        .collect()
}

/// Shared result validation: ownership, root confinement, hidden/ignore rules,
/// and bounded size. A provider can never widen the authorized search scope.
fn validated(entries: Vec<SearchEntry>, request: &PluginSearchInput) -> Result<Vec<Entry>, String> {
    let limit = request.limit();
    let ignores = crate::core::ignores::IgnoreSet::new(&request.ignores);
    let mut values = Vec::new();
    for item in entries.into_iter().take(limit) {
        let mut entry = core_entry(item);
        let relative = if crate::plugins::mounts::handles(&request.root) {
            let root = crate::plugins::mounts::parse(&request.root)?;
            let path = crate::plugins::mounts::parse(&entry.path)?;
            if root.mount_id != path.mount_id
                || !std::path::Path::new(&path.relative).starts_with(&root.relative)
            {
                return Err("search result outside requested mount".into());
            }
            std::path::Path::new(&path.relative)
                .strip_prefix(&root.relative)
                .unwrap_or_else(|_| std::path::Path::new(&entry.name))
                .to_path_buf()
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
            path.strip_prefix(&root)
                .unwrap_or_else(|_| std::path::Path::new(&entry.name))
                .to_path_buf()
        };
        if !request.hidden && entry.name.starts_with('.') {
            continue;
        }
        if ignores.is_ignored(&relative) {
            continue;
        }
        values.push(entry);
    }
    Ok(values)
}
