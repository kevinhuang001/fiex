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
        let listing = list_remote(&conn, parent)?;
        remember_listing(parent, &listing.files, &mut meta, listing.complete);
        save_meta(&conn, &meta)?;
    }
    let file = meta.files.get(&rel);
    if !rel.is_empty() && file.is_none() {
        return Err("remote node not found".into());
    }
    let state =
        super::super::cache::state(&conn, &meta, &rel, file.is_none_or(|entry| entry.is_dir));
    let pinned = state.pinned;
    let dirty = state.dirty;
    let has_cache = state.has_cache;
    let uploaded = state.uploaded;
    let name = relative
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| conn.name.clone());
    let allowed_ops = file
        .map(|entry| entry_capabilities(&plugin_id, entry.allowed_ops.as_deref()))
        .unwrap_or_else(|| entry_capabilities(&plugin_id, None));
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
        link_target: None,
        allowed_ops,
        tags: vec![],
        has_cache,
        pinned,
        uploaded,
        storage_projection: Default::default(),
        sync: Some(
            if state.conflict {
                "conflict"
            } else if dirty {
                "pending"
            } else if pinned {
                "pinned"
            } else if state.cached {
                "synced"
            } else if state.partial {
                "partial"
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
pub(crate) fn provider_node(
    path: &str,
) -> Result<(crate::plugins::providers::fs::Mount, String), String> {
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
        crate::plugins::providers::fs::Mount {
            mount_id: conn.mount_id.clone(),
            plugin_id,
            source: crate::plugins::providers::fs::Source::Connection(provider_config(&conn)),
            scope: None,
        },
        node,
    ))
}

/// Bind a committed opaque provider ID to the exact scoped destination. This
/// is also needed for newly created directories before their first listing.
pub(crate) fn record_provider_mutation(
    path: &str,
    entry: &crate::plugins::providers::fs::FsEntry,
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
    file.allowed_ops = entry
        .allowed_ops
        .as_ref()
        .map(|ops| ops.iter().map(|op| op.as_str().to_owned()).collect());
    save_meta(&connection, &metadata)
}

/// Record one completed provider listing in one metadata transaction. Listing
/// and stat refreshes update remote metadata only for uncached files; downloaded
/// bytes keep their revision baseline until reconciliation decides whether the
/// remote changed.
pub(crate) fn observe_provider_listing(
    directory: &str,
    entries: &[(String, crate::plugins::providers::fs::FsEntry)],
    complete: bool,
) -> Result<std::collections::HashMap<String, super::super::cache::CacheState>, String> {
    let (connection, directory_rel) =
        connection_for_path(directory).ok_or("StaleRef: connection removed")?;
    ensure_provider_access(&connection)?;
    let mut metadata = load_meta(&connection);
    let directory_relative = rel_string(&directory_rel);
    let inherited_pin = metadata.files.iter().any(|(key, file)| {
        file.pinned && file.is_dir && super::super::sync::in_tree(&directory_relative, key)
    });
    let mut projected_paths = Vec::with_capacity(entries.len());
    for (path, entry) in entries {
        let (candidate, relative) =
            connection_for_path(path).ok_or("StaleRef: connection removed")?;
        if candidate.id != connection.id {
            return Err("ScopeDenied: a listing cannot span connections".into());
        }
        let file = metadata.files.entry(rel_string(&relative)).or_default();
        if !file.downloaded {
            file.size = entry.size.unwrap_or(0);
            file.modified = entry.modified.unwrap_or(0).max(0) as u64;
            file.revision = entry.revision.clone().unwrap_or_default();
        }
        file.node_id = entry.node_id.clone();
        file.is_dir = entry.is_dir();
        file.allowed_ops = entry.allowed_ops.as_ref().map(|ops| {
            ops.iter()
                .map(|operation| operation.as_str().to_owned())
                .collect()
        });
        if inherited_pin && !file.excluded {
            file.pinned = true;
        }
        projected_paths.push((path.clone(), rel_string(&relative), entry.is_dir()));
    }
    if complete {
        metadata.listed.insert(directory_relative.clone());
        metadata.listed_children.insert(
            directory_relative,
            projected_paths
                .iter()
                .map(|(_, relative, _)| relative.clone())
                .collect(),
        );
    }
    save_meta(&connection, &metadata)?;
    let nodes = projected_paths
        .iter()
        .map(|(_, relative, is_dir)| (relative.as_str(), *is_dir))
        .collect::<Vec<_>>();
    let states = super::super::cache::states_for(&connection, &metadata, &nodes);
    Ok(projected_paths
        .into_iter()
        .filter_map(|(path, relative, _)| states.get(&relative).cloned().map(|state| (path, state)))
        .collect())
}
