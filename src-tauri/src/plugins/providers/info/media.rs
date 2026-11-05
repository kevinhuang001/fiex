use super::{localization, InfoSection};
use std::path::Path;

/// Core owns the shared media cache; `fiex-info` owns format probing.
pub fn media_sections(path: &Path) -> Vec<InfoSection> {
    let cache_key = crate::plugins::cache::key("info.media", path);
    if let Some(sections) = cache_key
        .as_deref()
        .and_then(crate::plugins::cache::get)
        .and_then(|value| serde_json::from_value(value).ok())
    {
        return sections;
    }
    let sections = fiex_info::media_sections(path, localization());
    if let Some(key) = cache_key {
        if let Ok(value) = serde_json::to_value(&sections) {
            crate::plugins::cache::put(key, value);
        }
    }
    sections
}
