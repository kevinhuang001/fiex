//! GitHub Release discovery and marketplace catalog parsing. A repository is
//! only a content source: every entry still goes through the same download,
//! manifest validation and explicit permission consent as a local package.

use crate::{plugins::host as plugin_host, plugins::security::permissions as plugin_permissions};
use serde::{Deserialize, Serialize};
use std::{io::Read, time::Duration};

mod catalog;
pub(crate) use catalog::*;

mod descriptions;
pub(crate) use descriptions::*;

#[cfg(test)]
mod tests;

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
