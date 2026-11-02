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
pub(crate) fn remember_provider_entry(
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
    file.readonly = entry.readonly.unwrap_or(false);
    file.allowed_ops = entry
        .allowed_ops
        .as_ref()
        .map(|ops| ops.iter().map(|op| op.as_str().to_owned()).collect());
    save_meta(&connection, &metadata)
}


/// Observe a complete unified VFS listing without treating it as a write.
/// A previous implementation re-used remember_provider_entry for list results,
/// which marked every downloaded file as a conflict on each refresh.
pub(crate) fn observe_connection_listing(
    directory: &str,
    entries: &[(String, crate::plugins::providers::fs::FsEntry)],
) -> Result<(), String> {
    let (conn, parent) =
        connection_for_path(directory).ok_or("StaleRef: connection removed")?;
    ensure_provider_access(&conn)?;
    let parent = rel_string(&parent);
    let mut meta = load_meta(&conn);
    let inherited_pin = meta.files.iter().any(|(key, file)| {
        file.pinned && file.is_dir && super::super::sync::in_tree(&parent, key)
    });
    for (path, entry) in entries {
        let uri = crate::plugins::mounts::parse(path)?;
        if uri.mount_id != conn.mount_id {
            return Err("listing contains a foreign mount".into());
        }
        let file = meta.files.entry(uri.relative).or_default();
        // An existing download owns its revision/baseline until reconciliation.
        if !file.downloaded {
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
        if !file.excluded {
            file.pinned |= inherited_pin;
        }
    }
    meta.listed.insert(parent);
    save_meta(&conn, &meta)
}

/// Core owns cache state; it does not belong to the plugin's VFS entry DTO.
/// Project this state back onto unified FS/VFS entries for badges and menus.
/// Plugin-created ephemeral views have no connection cache and are unchanged.
pub(crate) fn apply_connection_cache_state(entries: &mut [crate::core::files::Entry]) {
    let Some(first) = entries.first() else { return };
    let Some((conn, _)) = connection_for_path(&first.path) else {
        return;
    };
    let meta = load_meta(&conn);
    let cached_dirs = super::super::cache::cached_dirs(&meta);
    for entry in entries {
        let Ok(uri) = crate::plugins::mounts::parse(&entry.path) else {
            continue;
        };
        if uri.mount_id != conn.mount_id {
            continue;
        }
        let rel = uri.relative;
        let state = meta.files.get(&rel);
        let has_cache = if entry.is_dir {
            cached_dirs.contains(&rel)
        } else {
            state.is_some_and(|file| file.downloaded)
                && local_for(&conn, &rel).is_file()
        };
        let pinned = state.is_some_and(|file| file.pinned);
        let dirty = state.is_some_and(|file| super::super::sync::dirty(&conn, &rel, file));
        entry.has_cache = has_cache;
        entry.pinned = pinned;
        entry.uploaded = state.is_some_and(|file| file.uploaded && !file.conflict)
            && !dirty
            && (has_cache || pinned);
        entry.sync = Some(
            match state {
                Some(file) if file.conflict => "conflict",
                Some(_) if dirty => "pending",
                Some(_) if pinned => "pinned",
                Some(_) if has_cache => "synced",
                _ => "cloud",
            }
            .into(),
        );
    }
}
