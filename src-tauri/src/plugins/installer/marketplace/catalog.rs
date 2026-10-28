use super::*;

pub(crate) fn catalog_entries(
    value: &serde_json::Value,
    release_assets: &[serde_json::Value],
    release: &serde_json::Value,
    repository: &str,
    descriptions: &std::collections::HashMap<String, String>,
) -> Result<Vec<MarketplacePlugin>, String> {
    let platform = platform_key();
    let names = release_assets
        .iter()
        .filter_map(|asset| asset.get("name").and_then(serde_json::Value::as_str))
        .collect::<std::collections::HashSet<_>>();
    let plugins = value
        .get("plugins")
        .and_then(serde_json::Value::as_array)
        .ok_or("plugin catalog has no plugins array")?;
    let version = release
        .get("tag_name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let mut entries = Vec::new();
    for plugin in plugins {
        let id = plugin
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or("catalog plugin has no id")?;
        if !plugin_host::valid_id(id) {
            continue;
        }
        let Some(asset) = plugin
            .get("assets")
            .and_then(|assets| assets.get(&platform))
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        if !asset.ends_with(".fiexplugin") || asset.len() > 160 || !names.contains(asset) {
            continue;
        }
        let permissions = plugin
            .get("permissions")
            .and_then(serde_json::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .filter(|permission| plugin_permissions::PERMISSIONS.contains(permission))
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let commands = plugin
            .get("commands")
            .and_then(serde_json::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .filter(|command| plugin_permissions::valid_command(command))
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let requested_network_domains = plugin
            .get("networkDomains")
            .and_then(serde_json::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let declared_domains = permissions
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<String>>();
        let requested_network_domains = plugin_permissions::validate_network_domains(
            &declared_domains,
            &requested_network_domains,
        )
        .map(|set| set.into_iter().collect::<Vec<_>>())
        .unwrap_or_default();
        entries.push(MarketplacePlugin {
            id: id.into(),
            name: plugin
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(id)
                .into(),
            version: plugin
                .get("version")
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.is_empty())
                .unwrap_or(version)
                .into(),
            description: descriptions.get(id).cloned().unwrap_or_else(|| {
                plugin
                    .get("description")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            }),
            author: plugin
                .get("author")
                .cloned()
                .and_then(|value| serde_json::from_value(value).ok()),
            permissions,
            requested_network_domains,
            commands,
            extensions: plugin
                .get("extensions")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
            asset: asset.into(),
            repository: repository.into(),
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

pub(crate) fn fallback_marketplace(
    assets: &[serde_json::Value],
    release: &serde_json::Value,
    repository: &str,
) -> Result<Vec<MarketplacePlugin>, String> {
    let version = release
        .get("tag_name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let mut entries = Vec::new();
    for asset in assets {
        let Some(name) = asset.get("name").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if !name.ends_with(".fiexplugin") {
            continue;
        }
        let slug = name.split('-').next().unwrap_or("plugin");
        entries.push(MarketplacePlugin {
            id: format!("plugin.{slug}"),
            name: slug.into(),
            version: version.clone(),
            description: String::new(),
            author: None,
            permissions: Vec::new(),
            requested_network_domains: Vec::new(),
            commands: Vec::new(),
            extensions: serde_json::Value::Null,
            asset: name.into(),
            repository: repository.into(),
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}
