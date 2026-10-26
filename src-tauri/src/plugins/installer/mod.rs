//! Two-phase GitHub Release installer. No executable is started until the
//! host has displayed the full permission list and received explicit consent.
//! Download sources must be public GitHub Releases containing .fiexplugin ZIPs.
//! No plugin author/signature automatically grants permission.
//!
//! The installer is split into three concerns: `marketplace` discovers
//! releases and catalogs, `package` validates and unpacks the archive, and
//! `signing` applies the publisher trust gate. This module owns the pending
//! ticket state and the confirm/cancel lifecycle.
mod marketplace;
mod package;
mod signing;

pub use marketplace::{marketplace, release_assets, MarketplacePlugin};
pub use package::install_root;

use crate::{plugin_host, plugin_permissions};
use serde::Serialize;
use std::{
    collections::HashMap,
    fs,
    io::Read,
    path::Path,
    sync::{Mutex, OnceLock},
};
use tempfile::TempDir;

struct Staged {
    dir: TempDir,
    manifest: plugin_host::PluginManifest,
    verification: crate::plugin_signing::Verification,
    update: bool,
    previous_declared: Option<std::collections::BTreeSet<String>>,
}
fn pending() -> &'static Mutex<HashMap<String, Staged>> {
    static PENDING: OnceLock<Mutex<HashMap<String, Staged>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
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
    pub requested_network_all: bool,
    pub requested_network_domains: Vec<String>,
    pub commands: Vec<String>,
    pub repository: String,
    pub signed: bool,
    pub trusted: bool,
    pub publisher: Option<String>,
    pub publisher_name: Option<String>,
    pub key_id: Option<String>,
    pub publisher_key: Option<String>,
    pub fingerprint: Option<String>,
    pub is_update: bool,
    pub previous_version: Option<String>,
    pub permissions_changed: bool,
}

pub fn prepare(repository: &str, asset_name: &str) -> Result<InstallPreview, String> {
    if asset_name.len() > 160 || !asset_name.ends_with(".fiexplugin") {
        return Err("invalid package selection".into());
    }
    let (owner, repo) = marketplace::parse_repo(repository)?;
    let client = marketplace::request_client()?;
    let release = marketplace::plugin_release(repository, &client)?;
    let asset = marketplace::compatible_assets(&release)
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
        .unwrap_or(package::MAX_DOWNLOAD + 1)
        > package::MAX_DOWNLOAD
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
        .take(package::MAX_DOWNLOAD + 1)
        .read_to_end(&mut download)
        .map_err(|e| e.to_string())?;
    if download.len() as u64 > package::MAX_DOWNLOAD {
        return Err("plugin package exceeds size limit".into());
    }
    stage_package(download, repository)
}

fn stage_package(package: Vec<u8>, repository: &str) -> Result<InstallPreview, String> {
    let staging_parent = install_root()?.join(".staging");
    fs::create_dir_all(&staging_parent).map_err(|e| e.to_string())?;
    let staging = tempfile::Builder::new()
        .prefix("fiex-plugin-")
        .tempdir_in(&staging_parent)
        .map_err(|e| e.to_string())?;
    package::unpack(&package, staging.path())?;
    let (manifest_bytes, manifest) = package::read_manifest(staging.path())?;
    let verification = crate::plugin_signing::verify(&manifest_bytes, &manifest, staging.path())?;
    crate::plugin_resources::validate(&manifest, staging.path())?;
    crate::plugin_themes::load(&manifest, staging.path())?;
    let declared = plugin_host::declared_permissions(&manifest)?;
    let requested_network_all = manifest.permissions.network.all;
    let requested_network_domains = plugin_permissions::validate_network_domains(
        &declared,
        &manifest.permissions.network.domains,
    )?
    .into_iter()
    .collect::<Vec<_>>();
    let permissions = declared.clone().into_iter().collect::<Vec<_>>();
    let existing = install_root()?.join(&manifest.id);
    let previous_manifest = fs::read(existing.join("manifest.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<plugin_host::PluginManifest>(&bytes).ok());
    let previous_version = previous_manifest
        .as_ref()
        .map(|value| value.version.clone());
    let previous_declared = previous_manifest
        .as_ref()
        .and_then(|value| plugin_host::declared_permissions(value).ok());
    let update = existing.is_dir();
    // Make only the manifest-declared native executable runnable. No other
    // executable permission is accepted from the downloaded archive.
    if !crate::plugin_themes::is_theme(&manifest) {
        let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
        let current = manifest
            .runtime
            .entrypoints
            .get(&platform)
            .ok_or("plugin has no executable for this operating system / architecture")?;
        for relative in manifest.runtime.entrypoints.values() {
            if !package::normal_relative(Path::new(relative)) {
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
        requested_network_all,
        requested_network_domains,
        commands: manifest.permissions.commands.allow.clone(),
        repository: repository.into(),
        is_update: update,
        previous_version: previous_version.clone(),
        permissions_changed: update && previous_declared.as_ref() != Some(&declared),
        signed: verification.signed,
        trusted: verification.trusted,
        publisher: verification.publisher.clone(),
        publisher_name: manifest.publisher.as_ref().map(|value| value.name.clone()),
        key_id: verification.key_id.clone(),
        publisher_key: verification.publisher_key.clone(),
        fingerprint: signing::fingerprint(&verification),
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
            verification,
            update,
            previous_declared,
        },
    );
    Ok(info)
}

pub fn prepare_local(path: &str) -> Result<InstallPreview, String> {
    let path = Path::new(path);
    if path.extension().and_then(|value| value.to_str()) != Some("fiexplugin") {
        return Err("select a .fiexplugin package".into());
    }
    let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > package::MAX_DOWNLOAD {
        return Err("plugin package is too large".into());
    }
    let package = fs::read(path).map_err(|e| e.to_string())?;
    stage_package(package, "local import")
}

pub fn install(ticket: &str, allow_untrusted: bool) -> Result<String, String> {
    let mut pending = pending().lock().map_err(|_| "installer locked")?;
    let staged = pending.remove(ticket).ok_or("install preview expired")?;
    signing::trust_gate(&staged.verification, &staged.manifest, allow_untrusted)?;
    let id = staged.manifest.id.clone();
    let destination = install_root()?.join(&id);
    let parent = install_root()?;
    fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
    let declared = plugin_host::declared_permissions(&staged.manifest)?;
    let previous_declared = staged.previous_declared.clone();
    let backup = parent.join(format!(
        ".backup-{id}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or_default()
    ));
    crate::plugin_host::global().stop(&id);
    if destination.exists() {
        fs::rename(&destination, &backup).map_err(|e| e.to_string())?;
    }
    if let Err(error) = fs::rename(staged.dir.path(), &destination) {
        if destination.exists() {
            let _ = fs::remove_dir_all(&destination);
        }
        if backup.exists() {
            let _ = fs::rename(&backup, &destination);
        }
        return Err(error.to_string());
    }
    let permission_result = if staged.update {
        if previous_declared.as_ref() == Some(&declared) {
            // The declared surface did not change; keep the user's existing
            // activation and per-permission grants.
            Ok(())
        } else {
            // A changed permission surface must be reviewed again.
            plugin_permissions::revoke_install(&id).and_then(|_| {
                plugin_permissions::approve_install_with_domains(
                    &id,
                    &declared,
                    &staged.manifest.permissions.network.domains,
                    &staged.manifest.permissions.commands.allow,
                )
                .map(|_| ())
            })
        }
    } else {
        plugin_permissions::approve_install_with_domains(
            &id,
            &declared,
            &staged.manifest.permissions.network.domains,
            &staged.manifest.permissions.commands.allow,
        )
        .map(|_| ())
    };
    if let Err(error) = permission_result {
        let _ = fs::remove_dir_all(&destination);
        if backup.exists() {
            let _ = fs::rename(&backup, &destination);
        }
        return Err(error);
    }
    if backup.exists() {
        let _ = fs::remove_dir_all(&backup);
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
