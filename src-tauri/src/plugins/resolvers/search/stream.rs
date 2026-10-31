//! Incremental local SearchProvider pagination through the same verified
//! EventRouter and operation scope used by single-page requests.
use super::*;
use crate::plugin_sdk::provider_contracts::{ProviderEndpoint, ProviderRequest};
use crate::plugins::events::EventRouter;
use std::collections::HashSet;

const MAX_PAGES: usize = 64;

/// Kept for command-line integration fixtures; UI calls run_stream directly.
pub fn run(
    engine: &SearchEngineInfo,
    request: &PluginSearchInput,
    cancelled: impl Fn() -> bool,
) -> Result<(Vec<Entry>, bool), String> {
    let mut entries = Vec::new();
    let truncated = run_stream(engine, request, cancelled, &mut |entry| entries.push(entry))?;
    Ok((entries, truncated))
}

/// Emit each verified page before fetching the next one. Never merge results
/// from a different provider, and do not accept unbounded or repeated cursors.
pub fn run_stream(
    engine: &SearchEngineInfo,
    request: &PluginSearchInput,
    cancelled: impl Fn() -> bool,
    emit: &mut dyn FnMut(Entry),
) -> Result<bool, String> {
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

    let resources = resources(request);
    if resources.len() != 1 {
        return Err("search root could not be resolved to a trusted resource".into());
    }
    let mut cursor: Option<String> = None;
    let mut seen = HashSet::new();
    let mut count = 0usize;
    let mut truncated = false;
    for _ in 0..MAX_PAGES {
        if cancelled() {
            return Err("search cancelled".into());
        }
        let remaining = request.limit().saturating_sub(count);
        if remaining == 0 {
            return Ok(true);
        }
        let mut params = input(request, &resources);
        // Non-paged providers (including Everything) must still receive the full
        // remaining limit. Providers that expose nextCursor may return smaller pages.
        params["limit"] = json!(remaining);
        if let Some(current) = &cursor {
            params["cursor"] = json!(current);
        }
        let call = ProviderRequest {
            method: "search.query".into(),
            params,
        };
        let (entries, page_truncated, next) = EventRouter::resolve(
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
                // Decode the same strict page DTO used by the SDK.
                let page: fiex_provider_contracts::SearchResults =
                    serde_json::from_value(response.clone())
                        .map_err(|error| format!("invalid search page: {error}"))?;
                page.validate_page(remaining)?;
                let entries = validated(resolver, response, request)?;
                Ok((entries, page.truncated, page.next_cursor))
            },
            |_| true,
            |results| {
                results
                    .into_iter()
                    .next()
                    .ok_or_else(|| "search provider unavailable".to_string())?
                    .1
            },
        )?;
        truncated |= page_truncated;
        for entry in entries {
            if cancelled() {
                return Err("search cancelled".into());
            }
            emit(entry);
            count += 1;
        }
        match next {
            None => return Ok(truncated),
            Some(next) if !seen.insert(next.clone()) => {
                return Err("search pagination cursor did not advance".into());
            }
            Some(next) => cursor = Some(next),
        }
    }
    Err("search provider exceeded pagination budget".into())
}

