//! Durable, restart-safe reconciliation. Provider revisions are opaque and
//! providers enforce their protocol-specific revision checks. Failed uploads retain bytes.

use super::{cache::*, config::*, files::*, store::*, sync::*};
use serde_json::{json, Value};
use std::fs;

mod upload;
use upload::*;

fn invoke(conn: &StoredConnection, method: &str, mut params: Value) -> Result<Value, String> {
    let plugin = ensure_provider_access(conn)?;
    params["connection"] = provider_config(conn);
    params["mountId"] = json!(conn.mount_id);
    crate::plugins::host::global().invoke(&plugin, method, params)
}

fn stat(conn: &StoredConnection, rel: &str, file: &RemoteMetaFile) -> Result<Value, String> {
    invoke(
        conn,
        "vfs.stat",
        json!({"nodeId": if file.node_id.is_empty() { rel } else { &file.node_id }}),
    )
}

/// A provider write stream that has not committed yet. Dropping it (any error
/// on the way to `vfs.commit`) aborts the stream so unpublished bytes vanish and
/// the local snapshot/baseline stay untouched for a retry.
struct WriteLease<'a> {
    plugin: &'a str,
    mount_id: &'a str,
    stream: &'a str,
    finished: bool,
}

impl Drop for WriteLease<'_> {
    fn drop(&mut self) {
        if !self.finished {
            let _ = crate::plugins::host::global().invoke(
                self.plugin,
                "vfs.abort",
                json!({"mountId": self.mount_id, "streamId": self.stream}),
            );
        }
    }
}

fn reconcile_file(
    conn: &StoredConnection,
    rel: &str,
    pull: bool,
    changes: &mut Vec<CacheChange>,
) -> Result<(), String> {
    let mut meta = load_meta(conn);
    let Some(file) = meta.files.get(rel).cloned() else {
        return Ok(());
    };
    if file.is_dir || !file.downloaded || file.conflict {
        return Ok(());
    }
    let local = local_for(conn, rel);
    check_cache_path(conn, &local)?;
    if !local.is_file() {
        if pull {
            return Err(crate::i18n::t("缓存文件缺失").into());
        }
        // A vanished unpinned cache is forgotten; the next open downloads it.
        let entry = meta.files.get_mut(rel).unwrap();
        entry.downloaded = false;
        entry.uploaded = false;
        entry.baseline.clear();
        entry.local_stamp.clear();
        save_meta(conn, &meta)?;
        changes.push(cache_change(conn, rel, "uncached"));
        return Ok(());
    }
    let changed = dirty(conn, rel, &file);
    let remote = stat(conn, rel, &file)?;
    let revision = remote["revision"]
        .as_str()
        .filter(|v| !v.is_empty())
        .ok_or("provider has no revision support")?;
    if revision != file.revision {
        if changed {
            meta.files.get_mut(rel).unwrap().conflict = true;
            save_meta(conn, &meta)?;
            changes.push(cache_change(conn, rel, "conflict"));
            return Err(crate::i18n::t("本地与云端均有修改，请处理同步冲突").into());
        }
        if !pull {
            // Cache is push-only: a newer remote revision invalidates the copy
            // instead of being fetched behind the user's back.
            fs::remove_file(&local).map_err(|e| e.to_string())?;
            let entry = meta.files.get_mut(rel).unwrap();
            entry.downloaded = false;
            entry.uploaded = false;
            entry.baseline.clear();
            entry.local_stamp.clear();
            entry.revision = revision.into();
            save_meta(conn, &meta)?;
            changes.push(cache_change(conn, rel, "uncached"));
            return Ok(());
        }
        // Download to a separate sibling and recheck local contents before
        // replacement, so an editor save during network I/O is preserved.
        let staged = tempfile::NamedTempFile::new_in(local.parent().ok_or("invalid cache path")?)
            .map_err(|e| e.to_string())?;
        fetch_remote(
            conn,
            &file.node_id,
            staged.path(),
            &TransferOptions::default(),
        )?;
        if fingerprint(&local)? != file.baseline {
            return Err(crate::i18n::t("本地文件在下载期间改变，稍后重试").into());
        }
        let after = stat(conn, rel, &file)?;
        if after["revision"].as_str() != Some(revision) {
            return Err(crate::i18n::t("云端文件正在变化，稍后重试").into());
        }
        staged.persist(&local).map_err(|e| e.error.to_string())?;
        let entry = meta.files.get_mut(rel).unwrap();
        entry.revision = revision.into();
        entry.baseline = fingerprint(&local)?;
        entry.local_stamp = stamp(&local)?;
        entry.size = fs::metadata(&local).map_err(|e| e.to_string())?.len();
        entry.uploaded = false;
        save_meta(conn, &meta)?;
        changes.push(cache_change(conn, rel, "cached"));
    } else if changed {
        upload(conn, rel, &file)?;
        changes.push(cache_change(conn, rel, "uploaded"));
    }
    Ok(())
}

pub async fn reconcile() {
    for conn in load_stored() {
        let _ = reconcile_connection(&conn.id).await;
    }
}

pub async fn reconcile_connection(id: &str) -> Vec<CacheChange> {
    let mut changes = Vec::new();
    for conn in load_stored().into_iter().filter(|conn| conn.id == id) {
        if ensure_provider_access(&conn).is_err() {
            continue;
        }
        // The desktop scheduler runs separate connections independently.
        let _guard = lock(&conn).await;
        let before = downloaded_keys(&load_meta(&conn));
        let meta = load_meta(&conn);
        let pins: Vec<_> = meta
            .files
            .iter()
            .filter(|(key, f)| {
                f.pinned
                    && !meta.files.iter().any(|(other, parent)| {
                        other != *key && parent.pinned && parent.is_dir && in_tree(key, other)
                    })
            })
            .map(|(r, f)| (r.clone(), f.is_dir))
            .collect();
        for (rel, directory) in pins {
            let path = local_for(&conn, &rel).to_string_lossy().into_owned();
            let result = if directory {
                download_dir_inner(&path, &TransferOptions::default())
            } else {
                download_inner(&path, &TransferOptions::default()).map(|_| ())
            };
            record_result(&conn, &rel, result, &mut changes);
        }
        // Files a pinned folder pulled for the first time are cache changes too.
        for (key, file) in load_meta(&conn).files.iter() {
            if file.downloaded && !before.contains(key) {
                changes.push(cache_change(&conn, key, "cached"));
            }
        }
        let keys: Vec<_> = load_meta(&conn)
            .files
            .iter()
            .filter(|(_, file)| file.downloaded && !file.is_dir)
            .map(|(key, file)| (key.clone(), file.pinned))
            .collect();
        for (rel, pull) in keys {
            let result = reconcile_file(&conn, &rel, pull, &mut changes);
            record_result(&conn, &rel, result, &mut changes);
        }
    }
    changes
}

fn record_result(
    conn: &StoredConnection,
    rel: &str,
    result: Result<(), String>,
    changes: &mut Vec<CacheChange>,
) {
    let mut meta = load_meta(conn);
    match result {
        Ok(()) => {
            meta.errors.remove(rel);
        }
        Err(error) => {
            // Only a new or changed failure earns a fresh activity entry.
            let previous = meta.errors.get(rel).cloned();
            if previous.as_deref() != Some(error.as_str()) {
                changes.push(cache_failure(conn, rel, &error));
            }
            meta.errors.insert(rel.into(), error);
        }
    }
    let _ = save_meta(conn, &meta);
}

/// Explicit conflict resolution never silently discards the user's version.
pub async fn resolve(path: &str, keep_local: bool) -> Result<(), String> {
    let (conn, rel) = connection_for_path(path).ok_or("not a remote file")?;
    let _guard = lock(&conn).await;
    let rel = rel_string(&rel);
    let mut meta = load_meta(&conn);
    let file = meta.files.get(&rel).cloned().ok_or("cache entry missing")?;
    let current = stat(&conn, &rel, &file)?;
    let revision = current["revision"]
        .as_str()
        .filter(|r| !r.is_empty())
        .ok_or("missing revision")?;
    if !keep_local {
        let local = local_for(&conn, &rel);
        let mut backup = tempfile::Builder::new()
            .prefix("fiex-conflict-")
            .tempfile_in(local.parent().ok_or("invalid path")?)
            .map_err(|e| e.to_string())?;
        std::io::copy(
            &mut fs::File::open(&local).map_err(|e| e.to_string())?,
            &mut backup,
        )
        .map_err(|e| e.to_string())?;
        backup.keep().map_err(|e| e.error.to_string())?;
        // Baseline now represents the preserved local version; reconciliation
        // can safely replace it with the remote version.
        meta.files.get_mut(&rel).unwrap().baseline = fingerprint(&local)?;
    } else {
        meta.files.get_mut(&rel).unwrap().revision = revision.into();
    }
    meta.files.get_mut(&rel).unwrap().conflict = false;
    save_meta(&conn, &meta)?;
    reconcile_file(&conn, &rel, true, &mut Vec::new())
}

#[cfg(test)]
mod tests;
