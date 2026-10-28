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
        let drive = path.chars().next().ok_or(crate::i18n::t("无效设备"))?;
        let script = format!(
            "$d=(New-Object -ComObject Shell.Application).Namespace(17).ParseName('{drive}:');$d.InvokeVerb('Eject')"
        );
        let mut command = std::process::Command::new("powershell");
        command.args(["-NoProfile", "-Command", &script]);
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
        let status = command.status().map_err(|e| e.to_string())?;
        if status.success() {
            return Ok(());
        }
        return Err(crate::i18n::t("无法弹出该设备").into());
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
/// OS operations receive only a checked materialized cache path.
pub fn cached_path(path: &str) -> Result<String, String> {
    let (conn, rel) = connection_for_path(path).ok_or("MountNotFound: connection removed")?;
    ensure_provider_access(&conn)?;
    let local = local_for(&conn, &rel_string(&rel));
    super::super::config::check_cache_path(&conn, &local)?;
    Ok(local.to_string_lossy().into_owned())
}
