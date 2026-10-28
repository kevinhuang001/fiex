//! GitHub Release discovery and marketplace catalog parsing. A repository is
//! only a content source: every entry still goes through the same download,
//! manifest validation and explicit permission consent as a local package.
use crate::{plugins::host as plugin_host, plugins::security::permissions as plugin_permissions};
use serde::{Deserialize, Serialize};
use std::{io::Read, time::Duration};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MarketplacePlugin {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: Option<plugin_host::PluginAuthor>,
    pub permissions: Vec<String>,
    #[serde(default)]
    pub requested_network_domains: Vec<String>,
    pub commands: Vec<String>,
    #[serde(default)]
    pub extensions: serde_json::Value,
    pub asset: String,
    pub repository: String,
}
pub(super) fn parse_repo(value: &str) -> Result<(&str, &str), String> {
    let parts = value.trim().split('/').collect::<Vec<_>>();
    if parts.len() != 2
        || parts.iter().any(|p| {
            p.is_empty()
                || p.len() > 80
                || !p
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                || p.starts_with('.')
        })
    {
        return Err("enter a GitHub repository as owner/repo".into());
    }
    Ok((parts[0], parts[1]))
}
pub(super) fn request_client() -> Result<reqwest::blocking::Client, String> {
    let mut builder = reqwest::blocking::Client::builder()
        .user_agent("Fiex-Plugin-Installer/0.1")
        .timeout(Duration::from_secs(65));
    if let Some(proxy) = crate::platform::system_proxy::proxy_url() {
        builder = builder.proxy(reqwest::Proxy::all(proxy).map_err(|e| e.to_string())?);
    }
    builder.build().map_err(|e| e.to_string())
}
pub(super) fn compatible_assets(release: &serde_json::Value) -> Vec<&serde_json::Value> {
    let Some(all) = release.get("assets").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    let platform = platform_key();
    let matches = all
        .iter()
        .filter(|asset| {
            asset
                .get("name")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|name| name.ends_with(".fiexplugin") && name.contains(&platform))
        })
        .collect::<Vec<_>>();
    if !matches.is_empty() {
        return matches;
    }
    let generic = all
        .iter()
        .filter(|asset| {
            asset
                .get("name")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|name| name.ends_with(".fiexplugin"))
        })
        .collect::<Vec<_>>();
    if generic.len() == 1 {
        generic
    } else {
        Vec::new()
    }
}
pub(super) fn plugin_release(
    repository: &str,
    client: &reqwest::blocking::Client,
) -> Result<serde_json::Value, String> {
    let (owner, repo) = parse_repo(repository)?;
    let response = client
        .get(format!(
            "https://api.github.com/repos/{owner}/{repo}/releases?per_page=20"
        ))
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|e| format!("cannot list GitHub releases: {e}"))?;
    let releases: Vec<serde_json::Value> = response.json().map_err(|e| e.to_string())?;
    releases
        .into_iter()
        .find(|v| !compatible_assets(v).is_empty())
        .ok_or_else(|| "repository has no compatible .fiexplugin release assets".into())
}
pub fn release_assets(repository: &str) -> Result<Vec<String>, String> {
    let client = request_client()?;
    let release = plugin_release(repository, &client)?;
    let mut names = compatible_assets(&release)
        .into_iter()
        .filter_map(|a| {
            a.get("name")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .collect::<Vec<_>>();
    names.sort();
    Ok(names)
}
/// Official marketplace. The repository is only a content source: every
/// package still goes through the same download, manifest validation and
/// explicit permission consent as a manually supplied plugin.
pub fn marketplace(repository: &str) -> Result<Vec<MarketplacePlugin>, String> {
    let (owner, repo) = parse_repo(repository)?;
    let client = request_client()?;
    let release = plugin_release(repository, &client)?;
    let assets = release
        .get("assets")
        .and_then(serde_json::Value::as_array)
        .ok_or("release has no assets")?;
    let catalog = assets.iter().find(|asset| {
        asset
            .get("name")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|name| name == "fiex-catalog.json" || name == "catalog.json")
    });
    if let Some(catalog) = catalog {
        let url = catalog
            .get("browser_download_url")
            .and_then(serde_json::Value::as_str)
            .ok_or("catalog URL missing")?;
        if !url.starts_with(&format!(
            "https://github.com/{owner}/{repo}/releases/download/"
        )) {
            return Err("catalog URL has unexpected origin".into());
        }
        let response = client
            .get(url)
            .send()
            .and_then(reqwest::blocking::Response::error_for_status)
            .map_err(|e| format!("cannot download plugin catalog: {e}"))?;
        let mut bytes = Vec::new();
        response
            .take(2 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 2 * 1024 * 1024 {
            return Err("plugin catalog is too large".into());
        }
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|e| format!("plugin catalog invalid: {e}"))?;
        let descriptions = plugin_descriptions(assets, &client, owner, repo);
        return catalog_entries(&value, assets, &release, repository, &descriptions);
    }
    fallback_marketplace(assets, &release, repository)
}

fn platform_key() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

/// The release may ship one `plugin-descriptions.json` for every plugin; its
/// text is the store's introduction, so copy can change without repackaging.
fn plugin_descriptions(
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

fn parse_descriptions(bytes: &[u8]) -> std::collections::HashMap<String, String> {
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

fn catalog_entries(
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

fn fallback_marketplace(
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reject_untrusted_repository_names() {
        assert!(parse_repo("owner/repo").is_ok());
        for name in [
            "owner/repo/extra",
            "../repo",
            "owner/../repo",
            "owner/repo?x=1",
            "",
            "https://github.com/x/y",
        ] {
            assert!(parse_repo(name).is_err(), "{name}");
        }
    }
    #[test]
    fn release_listing_can_offer_multiple_plugins_per_platform() {
        let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
        let release = serde_json::json!({"assets":[
            {"name":format!("archive-{platform}.fiexplugin")},
            {"name":format!("git-{platform}.fiexplugin")},
            {"name":"other-platform-only.fiexplugin"}
        ]});
        assert_eq!(compatible_assets(&release).len(), 2);
    }
}

#[cfg(test)]
mod description_tests {
    use super::*;

    #[test]
    fn parses_release_descriptions_by_plugin_id() {
        let map = parse_descriptions(
            r#"{"schemaVersion":2,"plugins":[{"id":"org.fiex.git","description":"源代码管理"},{"id":"bad id","description":"x"}]}"#.as_bytes(),
        );
        assert_eq!(
            map.get("org.fiex.git").map(String::as_str),
            Some(crate::i18n::t("源代码管理"))
        );
        assert!(!map.contains_key("bad id"));
    }

    #[test]
    fn rejects_malformed_descriptions() {
        assert!(parse_descriptions(b"not json").is_empty());
    }
}
