use super::*;

pub async fn ensure_downloaded(path: &str, options: &TransferOptions) -> Result<String, String> {
    if connection_for_path(path).is_some() {
        download(path, options).await
    } else {
        crate::plugins::mounts::reject_local(path)?;
        Ok(path.into())
    }
}

pub fn eject(path: &str) -> Result<(), String> {
    crate::plugins::mounts::reject_local(path)?;
    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("diskutil")
            .args(["eject", path])
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            return Ok(());
        }
        return Err(crate::i18n::t("无法弹出该设备").into());
    }
    #[cfg(target_os = "linux")]
    {
        let status = std::process::Command::new("gio")
            .args(["mount", "-u", path])
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            return Ok(());
        }
        let status = std::process::Command::new("udisksctl")
            .args(["unmount", "-b", path])
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            return Ok(());
        }
        return Err(crate::i18n::t("无法卸载该设备").into());
    }
    #[cfg(target_os = "windows")]
    {
        return crate::platform::windows::eject_path(path);
    }
    #[allow(unreachable_code)]
    Err(crate::i18n::t("当前平台不支持弹出设备").into())
}
/// Metadata remains usable before a file has ever been downloaded.
pub fn stat(path: &str) -> Result<crate::core::files::Entry, String> {
    let (conn, relative) = connection_for_path(path).ok_or("MountNotFound: connection removed")?;
    let plugin_id = ensure_provider_access(&conn)?;
    let rel = rel_string(&relative);
    let mut meta = load_meta(&conn);
    if !rel.is_empty() && !meta.files.contains_key(&rel) {
        let parent = rel.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
        remember_listing(parent, &list_remote(&conn, parent)?, &mut meta);
        save_meta(&conn, &meta)?;
    }
    let file = meta.files.get(&rel);
    if !rel.is_empty() && file.is_none() {
        return Err("remote node not found".into());
    }
    let pinned = file.is_some_and(|entry| entry.pinned);
    let dirty = file.is_some_and(|entry| super::super::sync::dirty(&conn, &rel, entry));
    let has_cache = file.is_some_and(|entry| {
        if entry.is_dir {
            super::super::cache::subtree_has_cache(&meta, &rel)
        } else {
            entry.downloaded && local_for(&conn, &rel).is_file()
        }
    });
    let uploaded = file.is_some_and(|entry| entry.uploaded && !entry.conflict)
        && !dirty
        && (has_cache || pinned);
    let name = relative
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| conn.name.clone());
    let (readonly, allowed_ops) = file
        .map(|entry| entry_capabilities(&plugin_id, entry.allowed_ops.as_deref(), entry.readonly))
        .unwrap_or_else(|| entry_capabilities(&plugin_id, None, false));
    Ok(crate::core::files::Entry {
        path: super::super::config::navigation_path(&conn, &rel)?,
        name,
        is_dir: file.is_none_or(|m| m.is_dir),
        is_symlink: false,
        size: file.map(|m| m.size).unwrap_or(0),
        modified: file.map(|m| m.modified).unwrap_or(0),
        created: 0,
        extension: relative
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        mime: mime_guess::from_path(&relative)
            .first_or_octet_stream()
            .to_string(),
        readonly,
        link_target: None,
        allowed_ops,
        tags: vec![],
        has_cache,
        pinned,
        uploaded,
        sync: Some(
            if file.is_some_and(|entry| entry.conflict) {
                "conflict"
            } else if dirty {
                "pending"
            } else if pinned {
                "pinned"
            } else if has_cache {
                "synced"
            } else {
                "cloud"
            }
            .into(),
        ),
    })
}
/// Exact cache state for the UI: metadata says downloaded and the local file
/// still exists. Unlike the listing snapshot this cannot be stale after an
/// upload or a cache eviction.
#[cfg(feature = "desktop")]
pub fn is_cached(path: &str) -> Result<bool, String> {
    let (conn, rel) = connection_for_path(path).ok_or("MountNotFound: connection removed")?;
    ensure_provider_access(&conn)?;
    let meta = load_meta(&conn);
    Ok(meta.files.get(&rel_string(&rel)).is_some_and(|entry| {
        !entry.is_dir && entry.downloaded && local_for(&conn, &rel_string(&rel)).is_file()
    }))
}

/// OS operations receive only a checked materialized cache path.
pub fn cached_path(path: &str) -> Result<String, String> {
    let (conn, rel) = connection_for_path(path).ok_or("MountNotFound: connection removed")?;
    ensure_provider_access(&conn)?;
    let local = local_for(&conn, &rel_string(&rel));
    super::super::config::check_cache_path(&conn, &local)?;
    Ok(local.to_string_lossy().into_owned())
}

/// Resolve the provider's opaque ID, never a local download-cache path.
pub(crate) fn adapter_node(
    path: &str,
) -> Result<(crate::plugins::resolvers::fs::Mount, String), String> {
    let (conn, relative) = connection_for_path(path).ok_or("StaleRef: connection removed")?;
    let plugin_id = ensure_provider_access(&conn)?;
    let rel = rel_string(&relative);
    if !rel.is_empty() {
        stat(path)?;
    }
    let meta = load_meta(&conn);
    let node = if rel.is_empty() {
        String::new()
    } else {
        meta.files
            .get(&rel)
            .map(|entry| entry.node_id.clone())
            .filter(|id| !id.is_empty())
            .ok_or("StaleRef: provider node is unknown")?
    };
    Ok((
        crate::plugins::resolvers::fs::Mount {
            mount_id: conn.mount_id.clone(),
            plugin_id,
            source: crate::plugins::resolvers::fs::Source::Connection(provider_config(&conn)),
            scope: None,
        },
        node,
    ))
}

/// Bind a committed opaque provider ID to the exact scoped destination. This
/// is also needed for newly created directories before their first listing.
pub(crate) fn remember_adapter_entry(
    path: &str,
    entry: &crate::plugins::resolvers::fs::FsEntry,
) -> Result<(), String> {
    let (connection, relative) = connection_for_path(path).ok_or("StaleRef: connection removed")?;
    ensure_provider_access(&connection)?;
    let mut metadata = load_meta(&connection);
    let file = metadata.files.entry(rel_string(&relative)).or_default();
    // Preserve an existing local snapshot. A separate direct remote write
    // must not silently rebaseline or destroy possibly edited cached bytes.
    if file.downloaded {
        file.conflict = true;
    } else {
        file.size = entry.size.unwrap_or(0);
        file.modified = entry.modified.unwrap_or(0).max(0) as u64;
        file.revision = entry.revision.clone().unwrap_or_default();
    }
    file.node_id = entry.node_id.clone();
    file.is_dir = entry.is_dir();
    file.readonly = entry.readonly.unwrap_or(false);
    file.allowed_ops = entry
        .allowed_ops
        .as_ref()
        .map(|ops| ops.iter().map(|op| op.as_str().to_owned()).collect());
    save_meta(&connection, &metadata)
}
