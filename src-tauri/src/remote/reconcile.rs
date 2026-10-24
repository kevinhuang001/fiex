//! Durable, restart-safe reconciliation. Provider revisions are opaque and
//! providers enforce their protocol-specific revision checks. Failed uploads retain bytes.
use super::{config::*, files::*, store::*, sync::*};
use serde_json::{json, Value};
use std::fs;

fn invoke(conn: &StoredConnection, method: &str, mut params: Value) -> Result<Value, String> {
    let plugin = ensure_provider_access(conn)?;
    params["connection"] = provider_config(conn);
    crate::plugin_host::global().invoke(&plugin, method, params)
}

fn stat(conn: &StoredConnection, rel: &str, file: &RemoteMetaFile) -> Result<Value, String> {
    invoke(
        conn,
        "fs.stat",
        json!({"nodeId": if file.node_id.is_empty() { rel } else { &file.node_id }}),
    )
}

fn upload(conn: &StoredConnection, rel: &str, file: &RemoteMetaFile) -> Result<String, String> {
    let local = local_for(conn, rel);
    check_cache_path(conn, &local)?;
    // A stable disk snapshot prevents mixing two saves during a long upload.
    let mut snapshot = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
    let before = fingerprint(&local)?;
    std::io::copy(
        &mut fs::File::open(&local).map_err(|e| e.to_string())?,
        &mut snapshot,
    )
    .map_err(|e| e.to_string())?;
    if fingerprint(snapshot.path())? != before || fingerprint(&local)? != before {
        return Err("文件仍在保存，稍后重试".into());
    }
    if file.revision.is_empty() {
        return Err("提供者不支持版本条件写入，修改已保留在本地".into());
    }
    let size = snapshot
        .as_file()
        .metadata()
        .map_err(|e| e.to_string())?
        .len();
    let plugin = ensure_provider_access(conn)?;
    let source = snapshot
        .path()
        .file_name()
        .ok_or("invalid upload source")?
        .to_string_lossy()
        .into_owned();
    let node = if file.node_id.is_empty() {
        rel
    } else {
        &file.node_id
    };
    let result = crate::plugin_host::global().invoke_selected(&plugin, "fs.writeFile",
        json!({"connection":provider_config(conn), "nodeId":node, "source":source, "size":size, "ifRevision":file.revision}),
        snapshot.path().parent().ok_or("invalid upload source")?.to_str().ok_or("invalid upload path")?,
        &[snapshot.path().to_path_buf()])?;
    let revision = result["revision"]
        .as_str()
        .filter(|r| !r.is_empty())
        .ok_or("missing committed revision")?;
    let mut meta = load_meta(conn);
    let entry = meta.files.get_mut(rel).ok_or("cache entry disappeared")?;
    entry.revision = revision.into();
    entry.baseline = before;
    entry.local_stamp = if fingerprint(&local)? == entry.baseline {
        stamp(&local)?
    } else {
        String::new()
    };
    entry.size = size;
    entry.conflict = false;
    save_meta(conn, &meta)?;
    Ok(revision.into())
}

fn reconcile_file(conn: &StoredConnection, rel: &str) -> Result<(), String> {
    let mut meta = load_meta(conn);
    let Some(file) = meta.files.get(rel).cloned() else {
        return Ok(());
    };
    if file.is_dir || !file.downloaded || file.conflict {
        return Ok(());
    }
    check_cache_path(conn, &local_for(conn, rel))?;
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
            return Err("本地与云端均有修改，请处理同步冲突".into());
        }
        // Download to a separate sibling and recheck local contents before
        // replacement, so an editor save during network I/O is preserved.
        let local = local_for(conn, rel);
        let staged = tempfile::NamedTempFile::new_in(local.parent().ok_or("invalid cache path")?)
            .map_err(|e| e.to_string())?;
        fetch_remote(
            conn,
            &file.node_id,
            staged.path(),
            &TransferOptions::default(),
        )?;
        if fingerprint(&local)? != file.baseline {
            return Err("本地文件在下载期间改变，稍后重试".into());
        }
        let after = stat(conn, rel, &file)?;
        if after["revision"].as_str() != Some(revision) {
            return Err("云端文件正在变化，稍后重试".into());
        }
        staged.persist(&local).map_err(|e| e.error.to_string())?;
        let entry = meta.files.get_mut(rel).unwrap();
        entry.revision = revision.into();
        entry.baseline = fingerprint(&local)?;
        entry.local_stamp = stamp(&local)?;
        entry.size = fs::metadata(&local).map_err(|e| e.to_string())?.len();
        save_meta(conn, &meta)?;
    } else if changed {
        upload(conn, rel, &file)?;
    }
    Ok(())
}

pub async fn reconcile() {
    for conn in load_stored() {
        reconcile_connection(&conn.id).await;
    }
}

pub async fn reconcile_connection(id: &str) {
    for conn in load_stored().into_iter().filter(|conn| conn.id == id) {
        if ensure_provider_access(&conn).is_err() {
            continue;
        }
        // The desktop scheduler runs separate connections independently.
        let _guard = lock(&conn).await;
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
            record_result(&conn, &rel, result);
        }
        let keys: Vec<_> = load_meta(&conn)
            .files
            .iter()
            .filter(|(_, file)| file.downloaded && !file.is_dir)
            .map(|(key, _)| key.clone())
            .collect();
        for rel in keys {
            record_result(&conn, &rel, reconcile_file(&conn, &rel));
        }
    }
}

fn record_result(conn: &StoredConnection, rel: &str, result: Result<(), String>) {
    let mut meta = load_meta(conn);
    match result {
        Ok(()) => {
            meta.errors.remove(rel);
        }
        Err(error) => {
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
    reconcile_file(&conn, &rel)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn subtree_boundaries() {
        assert!(in_tree("a/b", "a"));
        assert!(!in_tree("ab/file", "a"));
        assert!(in_tree("anything", ""));
    }
    #[test]
    fn fingerprints_include_content() {
        let file = tempfile::NamedTempFile::new().unwrap();
        fs::write(file.path(), b"one").unwrap();
        let first = fingerprint(file.path()).unwrap();
        fs::write(file.path(), b"two").unwrap();
        assert_ne!(first, fingerprint(file.path()).unwrap());
    }
    #[test]
    fn metadata_migration_defaults() {
        let file: RemoteMetaFile =
            serde_json::from_str(r#"{"size":1,"modified":0,"isDir":false,"downloaded":true}"#)
                .unwrap();
        assert!(file.baseline.is_empty());
        assert!(!file.pinned);
    }
}
