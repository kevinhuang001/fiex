use super::*;

pub async fn download(path: &str, options: &TransferOptions) -> Result<String, String> {
    let Some((conn, _)) = connection_for_path(path) else {
        crate::plugins::mounts::reject_local(path)?;
        return Ok(path.into());
    };
    let _guard = super::super::sync::lock(&conn).await;
    download_inner(path, options)
}

pub(crate) fn download_inner(path: &str, options: &TransferOptions) -> Result<String, String> {
    let Some((conn, rel)) = connection_for_path(path) else {
        crate::plugins::mounts::reject_local(path)?;
        return Ok(path.into());
    };
    ensure_provider_access(&conn)?;
    let rel = rel_string(&rel);
    let mut meta = load_meta(&conn);
    let local = local_for(&conn, &rel);
    if local.is_file() && meta.files.get(&rel).is_some_and(|m| m.downloaded) {
        return Ok(local.to_string_lossy().into_owned());
    }
    if local.is_file() {
        return Err(crate::i18n::t("本地已有未登记的文件，已保留；请先备份或处理同步冲突").into());
    }
    if !meta.files.contains_key(&rel) {
        let parent = rel.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
        let listing = list_remote(&conn, parent)?;
        remember_listing(parent, &listing, &mut meta);
    }
    if meta.files.get(&rel).is_some_and(|f| f.revision.is_empty()) {
        let file = &meta.files[&rel];
        if let Ok(info) = crate::plugins::host::global().invoke(
            &ensure_provider_access(&conn)?,
            "vfs.stat",
            serde_json::json!({
                "mountId": conn.mount_id,
                "connection": provider_config(&conn),
                "nodeId": if file.node_id.is_empty() { &rel } else { &file.node_id }
            }),
        ) {
            meta.files.get_mut(&rel).unwrap().revision =
                info["revision"].as_str().unwrap_or("").into();
        }
    }
    let entry = meta
        .files
        .get(&rel)
        .ok_or(crate::i18n::t("远程文件不存在"))?
        .clone();
    if entry.is_dir {
        return Err(crate::i18n::t("不能作为文件打开文件夹").into());
    }
    let node = if entry.node_id.is_empty() {
        &rel
    } else {
        &entry.node_id
    };
    // The listing already contains content length. Forward it through the
    // generic openRead contract; WebDAV then avoids another PROPFIND request.
    // A zero-sized cached entry may mean "unknown" in legacy metadata.
    fetch_remote_with_size(
        &conn,
        node,
        &local,
        options,
        (entry.size > 0).then_some(entry.size),
    )?;
    let file = meta.files.get_mut(&rel).unwrap();
    file.downloaded = true;
    file.uploaded = false;
    file.baseline = super::super::sync::fingerprint(&local)?;
    file.local_stamp = super::super::sync::stamp(&local)?;
    save_meta(&conn, &meta)?;
    Ok(local.to_string_lossy().into_owned())
}

pub async fn download_dir(path: &str, options: &TransferOptions) -> Result<(), String> {
    let Some((conn, _)) = connection_for_path(path) else {
        return Ok(());
    };
    let _guard = super::super::sync::lock(&conn).await;
    download_dir_inner(path, options)
}

pub(crate) fn download_dir_inner(path: &str, options: &TransferOptions) -> Result<(), String> {
    let Some((conn, rel)) = connection_for_path(path) else {
        return Ok(());
    };
    let mut pending = vec![rel_string(&rel)];
    while let Some(current) = pending.pop() {
        if options.cancelled() {
            return Err(crate::core::operations::CANCELLED.into());
        }
        let mut meta = load_meta(&conn);
        let files = list_remote(&conn, &current)?;
        remember_listing(&current, &files, &mut meta);
        fs::create_dir_all(local_for(&conn, &current)).map_err(|e| e.to_string())?;
        save_meta(&conn, &meta)?;
        for file in files {
            let child = child_path(&current, &file.name);
            if meta.files.get(&child).is_some_and(|f| f.excluded) {
                continue;
            }
            if file.is_dir {
                pending.push(child);
            } else {
                download_inner(&local_for(&conn, &child).to_string_lossy(), options)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn child_path(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.into()
    } else {
        format!("{parent}/{name}")
    }
}

pub(crate) fn remember_listing(
    parent: &str,
    files: &[RemoteFile],
    meta: &mut super::super::sync::RemoteMeta,
) {
    meta.listed.insert(parent.into());
    let inherited = meta
        .files
        .iter()
        .any(|(key, file)| file.pinned && file.is_dir && super::super::sync::in_tree(parent, key));
    // A cached file's revision remains its download baseline. Listing must not
    // silently advance that baseline before reconciliation checks for conflicts.
    for file in files {
        let child = child_path(parent, &file.name);
        let old = meta.files.entry(child).or_default();
        if !old.downloaded {
            old.size = file.size;
            old.modified = file.modified;
            old.revision = file.revision.clone();
        }
        old.node_id = file.node_id.clone();
        old.allowed_ops = file.allowed_ops.clone();
        old.readonly = file.readonly;
        old.is_dir = file.is_dir;
        if !old.excluded {
            old.pinned |= inherited;
        }
    }
}

pub(super) fn cached_listing(
    parent: &str,
    meta: &super::super::sync::RemoteMeta,
) -> Vec<RemoteFile> {
    meta.files
        .iter()
        .filter_map(|(key, m)| {
            let (dir, name) = key.rsplit_once('/').unwrap_or(("", key));
            (dir == parent).then(|| RemoteFile {
                name: name.into(),
                is_dir: m.is_dir,
                size: m.size,
                modified: m.modified,
                node_id: m.node_id.clone(),
                revision: m.revision.clone(),
                allowed_ops: m.allowed_ops.clone(),
                readonly: m.readonly,
            })
        })
        .collect()
}
