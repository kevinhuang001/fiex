//! Two-phase GitHub Release installer. No executable is started until the
//! host has displayed the full permission list and received explicit consent.
//! Download sources must be public GitHub Releases containing .fiexplugin ZIPs.
//! No plugin author/signature automatically grants permission.
//!
//! The installer is split into three concerns: `marketplace` discovers
//! releases and catalogs, `package` validates and unpacks the archive, and
//! `signing` applies the publisher trust gate. This module owns the pending
//! ticket state and the confirm/cancel lifecycle.

mod staging;
pub use staging::*;

mod marketplace;

mod package;

mod signing;

pub use marketplace::{marketplace, release_assets, MarketplacePlugin};

pub use package::install_root;

use crate::{plugins::host as plugin_host, plugins::security::permissions as plugin_permissions};

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
    verification: crate::plugins::signing::Verification,
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
    pub description: String,
    pub icon: Option<String>,
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
    crate::plugins::host::global().stop(&id);
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
    plugin_host::invalidate_discovery();
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
    fs::remove_dir_all(directory).map_err(|e| e.to_string())?;
    plugin_host::invalidate_discovery();
    Ok(())
}
