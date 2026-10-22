//! Two-phase GitHub Release installer. No executable is started until the
//! host has displayed the full permission list and received explicit consent.
//! Download sources must be public GitHub Releases containing .fiexplugin ZIPs.
//! No plugin author/signature automatically grants permission.
use crate::{plugin_host, plugin_permissions};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    io::{Cursor, Read, Write},
    path::{Component, Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::Duration,
};
use tempfile::TempDir;
use zip::ZipArchive;

const MAX_DOWNLOAD: u64 = 60 * 1024 * 1024;
const MAX_UNPACKED: u64 = 256 * 1024 * 1024;
const MAX_FILES: usize = 2_000;

struct Staged {
    dir: TempDir,
    manifest: plugin_host::PluginManifest,
}
fn pending() -> &'static Mutex<HashMap<String, Staged>> {
    static PENDING: OnceLock<Mutex<HashMap<String, Staged>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}
pub fn install_root() -> Result<PathBuf, String> {
    dirs::data_local_dir()
        .map(|p| p.join("fiex").join("plugins"))
        .ok_or("application data directory unavailable".into())
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPreview {
    pub ticket: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: Option<plugin_host::PluginAuthor>,
    pub permissions: Vec<String>,
    pub requested_network_domains: Vec<String>,
    pub repository: String,
}

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
    pub asset: String,
    pub repository: String,
}
fn parse_repo(value: &str) -> Result<(&str, &str), String> {
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
fn normal_relative(path: &Path) -> bool {
    path.components().all(|c| matches!(c, Component::Normal(_)))
}
fn request_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent("Fiex-Plugin-Installer/0.1")
        .timeout(Duration::from_secs(65))
        .build()
        .map_err(|e| e.to_string())
}
fn unpack(package: &[u8], output: &Path) -> Result<(), String> {
    let reader = Cursor::new(package);
    let mut zip = ZipArchive::new(reader).map_err(|e| format!("invalid plugin ZIP: {e}"))?;
    if zip.len() > MAX_FILES {
        return Err("plugin contains too many files".into());
    }
    let mut unpacked = 0u64;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let rel = entry
            .enclosed_name()
            .ok_or("unsafe ZIP member path")?
            .to_path_buf();
        if !normal_relative(&rel) {
            return Err("ZIP member escapes plugin directory".into());
        }
        // Disallow symbolic links, devices and other special file types.
        if let Some(mode) = entry.unix_mode() {
            let kind = mode & 0o170000;
            if kind != 0 && kind != 0o100000 && kind != 0o040000 {
                return Err("plugin ZIP contains a symlink or special file".into());
            }
        }
        unpacked = unpacked
            .checked_add(entry.size())
            .ok_or("plugin size overflow")?;
        if unpacked > MAX_UNPACKED {
            return Err("plugin unpacked data too large".into());
        }
        let destination = output.join(rel);
        if entry.is_dir() {
            fs::create_dir_all(&destination).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut file = fs::File::create(&destination).map_err(|e| e.to_string())?;
            let count = std::io::copy(&mut entry, &mut file).map_err(|e| e.to_string())?;
            if count != entry.size() {
                return Err("incomplete plugin ZIP member".into());
            }
            file.flush().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
fn compatible_assets(release: &serde_json::Value) -> Vec<&serde_json::Value> {
    let Some(all) = release.get("assets").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
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
fn plugin_release(
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
        return catalog_entries(&value, assets, &release, repository);
    }
    fallback_marketplace(assets, &release, repository)
}

fn platform_key() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

fn catalog_entries(
    value: &serde_json::Value,
    release_assets: &[serde_json::Value],
    release: &serde_json::Value,
    repository: &str,
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
            description: plugin
                .get("description")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .into(),
            author: plugin
                .get("author")
                .cloned()
                .and_then(|value| serde_json::from_value(value).ok()),
            permissions,
            requested_network_domains,
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
            id: format!("org.fiex.{slug}"),
            name: slug.into(),
            version: version.clone(),
            description: String::new(),
            author: None,
            permissions: Vec::new(),
            requested_network_domains: Vec::new(),
            asset: name.into(),
            repository: repository.into(),
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

pub fn prepare(repository: &str, asset_name: &str) -> Result<InstallPreview, String> {
    if asset_name.len() > 160 || !asset_name.ends_with(".fiexplugin") {
        return Err("invalid package selection".into());
    }
    let (owner, repo) = parse_repo(repository)?;
    let client = request_client()?;
    let release = plugin_release(repository, &client)?;
    let asset = compatible_assets(&release)
        .into_iter()
        .find(|a| a.get("name").and_then(serde_json::Value::as_str) == Some(asset_name))
        .ok_or("requested plugin asset is unavailable for this platform")?;
    let url = asset
        .get("browser_download_url")
        .and_then(serde_json::Value::as_str)
        .ok_or("release asset URL missing")?;
    if !url.starts_with(&format!(
        "https://github.com/{owner}/{repo}/releases/download/"
    )) {
        return Err("release asset URL has unexpected origin".into());
    }
    if asset
        .get("size")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(MAX_DOWNLOAD + 1)
        > MAX_DOWNLOAD
    {
        return Err("plugin download is too large".into());
    }
    let response = client
        .get(url)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|e| format!("plugin download failed: {e}"))?;
    let mut download = Vec::new();
    response
        .take(MAX_DOWNLOAD + 1)
        .read_to_end(&mut download)
        .map_err(|e| e.to_string())?;
    if download.len() as u64 > MAX_DOWNLOAD {
        return Err("plugin package exceeds size limit".into());
    }
    let staging_parent = install_root()?.join(".staging");
    fs::create_dir_all(&staging_parent).map_err(|e| e.to_string())?;
    let staging = tempfile::Builder::new()
        .prefix("fiex-plugin-")
        .tempdir_in(&staging_parent)
        .map_err(|e| e.to_string())?;
    unpack(&download, staging.path())?;
    let manifest_bytes = fs::read(staging.path().join("manifest.json"))
        .map_err(|_| "manifest.json must be at the ZIP root")?;
    if manifest_bytes.len() > 1024 * 1024 {
        return Err("manifest too large".into());
    }
    let manifest: plugin_host::PluginManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|e| format!("manifest invalid: {e}"))?;
    if !plugin_host::valid_id(&manifest.id) {
        return Err("invalid plugin id".into());
    }
    let declared = plugin_permissions::declaration(&manifest.permissions)?;
    let requested_network_domains =
        plugin_permissions::validate_network_domains(&declared, &manifest.network_domains)?
            .into_iter()
            .collect::<Vec<_>>();
    let permissions = declared.into_iter().collect::<Vec<_>>();
    if install_root()?.join(&manifest.id).exists() {
        return Err("plugin already installed; uninstall before installing another version".into());
    }
    // Make only the manifest-declared native executable runnable. No other
    // executable permission is accepted from the downloaded archive.
    let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
    let current = manifest
        .entrypoints
        .get(&platform)
        .ok_or("plugin has no executable for this operating system / architecture")?;
    for relative in manifest.entrypoints.values() {
        if !normal_relative(Path::new(relative)) {
            return Err("unsafe executable path".into());
        }
    }
    let executable = staging.path().join(current);
    if !executable.is_file() {
        return Err("missing native backend for current platform".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
    }
    let ticket = staging
        .path()
        .file_name()
        .ok_or("missing install ticket")?
        .to_string_lossy()
        .to_string();
    let info = InstallPreview {
        ticket: ticket.clone(),
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        author: manifest.author.clone(),
        permissions,
        requested_network_domains,
        repository: repository.into(),
    };
    let mut map = pending().lock().map_err(|_| "installer locked")?;
    if map.len() >= 8 {
        return Err("too many pending installations".into());
    }
    map.insert(
        ticket,
        Staged {
            dir: staging,
            manifest,
        },
    );
    Ok(info)
}
pub fn install(ticket: &str) -> Result<String, String> {
    let mut pending = pending().lock().map_err(|_| "installer locked")?;
    let staged = pending.remove(ticket).ok_or("install preview expired")?;
    let id = staged.manifest.id.clone();
    let destination = install_root()?.join(&id);
    if destination.exists() {
        return Err("plugin is already installed".into());
    }
    let declared = plugin_permissions::declaration(&staged.manifest.permissions)?;
    let parent = install_root()?;
    fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
    fs::rename(staged.dir.path(), &destination).map_err(|e| e.to_string())?;
    // If persistence of the approval fails, remove the newly installed files.
    if let Err(error) = plugin_permissions::approve_install_with_domains(
        &id,
        &declared,
        &staged.manifest.network_domains,
    ) {
        let _ = fs::remove_dir_all(&destination);
        return Err(error);
    }
    Ok(id)
}
pub fn cancel(ticket: &str) -> Result<(), String> {
    pending()
        .lock()
        .map_err(|_| "installer locked")?
        .remove(ticket);
    Ok(())
}
pub fn uninstall(id: &str) -> Result<(), String> {
    if !plugin_host::valid_id(id) {
        return Err("invalid plugin id".into());
    }
    let directory = install_root()?.join(id);
    if !directory.is_dir() {
        return Err("plugin is not installed".into());
    }
    plugin_host::global().stop(id);
    // Remove approval FIRST: even on deletion failure, plugin cannot launch.
    plugin_permissions::revoke_install(id)?;
    fs::remove_dir_all(directory).map_err(|e| e.to_string())
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
    #[test]
    fn zip_slip_is_not_accepted() {
        assert!(!normal_relative(Path::new("../outside")));
        assert!(!normal_relative(Path::new("/etc/passwd")));
    }
}
