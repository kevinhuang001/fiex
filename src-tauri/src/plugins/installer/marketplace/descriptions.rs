use super::*;
/// The release may ship one `plugin-descriptions.json` for every plugin; its
/// text is the store's introduction, so copy can change without repackaging.
pub(crate) fn plugin_descriptions(
    assets: &[serde_json::Value],
    client: &reqwest::blocking::Client,
    owner: &str,
    repo: &str,
) -> std::collections::HashMap<String, String> {
    let Some(asset) = assets.iter().find(|asset| {
        asset.get("name").and_then(serde_json::Value::as_str) == Some("plugin-descriptions.json")
    }) else {
        return Default::default();
    };
    let Some(url) = asset
        .get("browser_download_url")
        .and_then(serde_json::Value::as_str)
    else {
        return Default::default();
    };
    if !url.starts_with(&format!(
        "https://github.com/{owner}/{repo}/releases/download/"
    )) {
        return Default::default();
    }
    let Ok(response) = client
        .get(url)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
    else {
        return Default::default();
    };
    let mut bytes = Vec::new();
    if response.take(512 * 1024).read_to_end(&mut bytes).is_err() {
        return Default::default();
    }
    parse_descriptions(&bytes)
}

pub(crate) fn parse_descriptions(bytes: &[u8]) -> std::collections::HashMap<String, String> {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return Default::default();
    };
    value
        .get("plugins")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let id = item.get("id").and_then(serde_json::Value::as_str)?;
                    if !plugin_host::valid_id(id) {
                        return None;
                    }
                    let text = item
                        .get("description")
                        .and_then(serde_json::Value::as_str)?;
                    Some((id.to_string(), text.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}
