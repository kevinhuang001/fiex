use super::*;

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

pub(super) fn stage_package(package: Vec<u8>, repository: &str) -> Result<InstallPreview, String> {
    let staging_parent = install_root()?.join(".staging");
    fs::create_dir_all(&staging_parent).map_err(|e| e.to_string())?;
    let staging = tempfile::Builder::new()
        .prefix("fiex-plugin-")
        .tempdir_in(&staging_parent)
        .map_err(|e| e.to_string())?;
    package::unpack(&package, staging.path())?;
    let (manifest_bytes, manifest) = package::read_manifest(staging.path())?;
    let verification = crate::plugins::signing::verify(&manifest_bytes, &manifest, staging.path())?;
    crate::plugins::resources::validate(&manifest, staging.path())?;
    crate::plugins::themes::load(&manifest, staging.path())?;
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
    if !crate::plugins::themes::is_theme(&manifest) {
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
        description: manifest.description.clone(),
        icon: manifest.icon.clone(),
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
