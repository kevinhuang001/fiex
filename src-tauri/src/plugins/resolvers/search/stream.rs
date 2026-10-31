//! Incremental local SearchProvider pagination through the same verified
//! EventRouter and operation scope used by single-page requests.
use super::*;
use std::collections::HashSet;

const PAGE_SIZE: usize = 100;
const MAX_PAGES: usize = 64;
const MAX_CURSOR_BYTES: usize = 2048;

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
        let mut params = input(request);
        params["limit"] = json!(remaining.min(PAGE_SIZE));
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
                let page_truncated = parse_truncated(&response)?;
                let next = parse_cursor(&response)?;
                let entries = validated(resolver, response, request)?;
                if entries.len() > remaining.min(PAGE_SIZE) {
                    return Err("search provider exceeded requested page limit".into());
                }
                Ok((entries, page_truncated, next))
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

fn parse_truncated(response: &Value) -> Result<bool, String> {
    match response.get("truncated") {
        None => Ok(false),
        Some(Value::Bool(value)) => Ok(*value),
        _ => Err("invalid search response: truncated must be a boolean".into()),
    }
}

fn parse_cursor(response: &Value) -> Result<Option<String>, String> {
    match response.get("nextCursor") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(cursor))
            if !cursor.is_empty() && cursor.len() <= MAX_CURSOR_BYTES =>
        {
            Ok(Some(cursor.clone()))
        }
        _ => Err("invalid search pagination cursor".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sdk_omitted_false_is_valid_but_malformed_flags_fail() {
        assert!(!parse_truncated(&json!({"entries":[]})).unwrap());
        assert!(!parse_truncated(&json!({"truncated":false})).unwrap());
        assert!(parse_truncated(&json!({"truncated":true})).unwrap());
        for invalid in [json!({"truncated":null}), json!({"truncated":"false"})] {
            assert!(parse_truncated(&invalid).is_err());
        }
    }

    #[test]
    fn pagination_cursors_must_be_bounded_and_typed() {
        assert_eq!(parse_cursor(&json!({})).unwrap(), None);
        assert_eq!(parse_cursor(&json!({"nextCursor":"p2"})).unwrap(), Some("p2".into()));
        for invalid in [
            json!({"nextCursor":""}),
            json!({"nextCursor":true}),
            json!({"nextCursor":"x".repeat(MAX_CURSOR_BYTES + 1)}),
        ] {
            assert!(parse_cursor(&invalid).is_err());
        }
    }
}
