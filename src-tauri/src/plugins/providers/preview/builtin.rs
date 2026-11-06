use std::path::Path;

pub(super) use fiex_preview::Builtin;

pub(super) fn builtin_view(
    kind: Builtin,
    local: &Path,
    _display: &str,
) -> Result<serde_json::Value, String> {
    let cache_key = (kind == Builtin::Text)
        .then(|| crate::plugins::cache::key("preview.text", local))
        .flatten();
    if let Some(value) = cache_key.as_deref().and_then(crate::plugins::cache::get) {
        return Ok(value);
    }
    let view = fiex_preview::render(kind, local, crate::plugins::match_rules::mime_for)?;
    if let Some(key) = cache_key {
        crate::plugins::cache::put(key, view.clone());
    }
    Ok(view)
}
