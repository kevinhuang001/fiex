//! Download bookkeeping plus pin, uncache and conflict helpers.

#[path = "sync/metadata.rs"]
mod metadata;
pub(crate) use metadata::{
    RemoteMeta, RemoteMetaFile, forget_meta, load_meta, load_meta_from, save_meta, save_meta_to,
};

use std::{collections::HashMap, fs, path::Path};

use super::{
    config::{connection_for_path, ensure_provider_access, local_for, rel_string},
    files::{TransferOptions, download_dir_inner, download_inner},
    store::StoredConnection,
};

type SelectedConnection = (StoredConnection, Vec<(String, String)>);

fn selected_connection(paths: &[String]) -> Result<Option<SelectedConnection>, String> {
    let Some(first_path) = paths.first() else {
        return Ok(None);
    };
    let (connection, relative) =
        connection_for_path(first_path).ok_or("MountNotFound: connection removed")?;
    ensure_provider_access(&connection)?;
    let identity = (
        connection.id.clone(),
        connection.mount_id.clone(),
        connection.cache_revision.clone(),
    );
    let mut roots = vec![(first_path.clone(), rel_string(&relative))];
    for path in &paths[1..] {
        let (candidate, relative) =
            connection_for_path(path).ok_or("MountNotFound: connection removed")?;
        ensure_provider_access(&candidate)?;
        if (
            candidate.id.clone(),
            candidate.mount_id.clone(),
            candidate.cache_revision.clone(),
        ) != identity
        {
            return Err("Unsupported: cache actions require one current Connection".into());
        }
        let root = rel_string(&relative);
        if !roots.iter().any(|(_, existing)| existing == &root) {
            roots.push((path.clone(), root));
        }
    }
    Ok(Some((connection, roots)))
}

fn validate_roots(meta: &RemoteMeta, roots: &[(String, String)]) -> Result<(), String> {
    if roots
        .iter()
        .any(|(_, root)| !root.is_empty() && !meta.files.contains_key(root))
    {
        return Err(crate::i18n::t("请先刷新远程目录").into());
    }
    Ok(())
}

// Per-connection serialization includes metadata, materialization, eviction and
// reconciliation. No lock is held while waiting for another connection.
pub(crate) async fn lock(conn: &StoredConnection) -> tokio::sync::OwnedMutexGuard<()> {
    use std::sync::{Arc, Mutex, OnceLock};
    static LOCKS: OnceLock<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> = OnceLock::new();
    let gate = LOCKS
        .get_or_init(Default::default)
        .lock()
        .unwrap()
        .entry(conn.id.clone())
        .or_default()
        .clone();
    gate.lock_owned().await
}

pub(crate) fn fingerprint(path: &std::path::Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buf = [0u8; 256 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    Ok(hex::encode(hash.finalize()))
}

pub(crate) fn stamp(path: &std::path::Path) -> Result<String, String> {
    let m = fs::metadata(path).map_err(|e| e.to_string())?;
    let modified = m
        .modified()
        .map_err(|e| e.to_string())?
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    Ok(format!("{}:{modified}", m.len()))
}

pub(crate) fn dirty(conn: &StoredConnection, rel: &str, file: &RemoteMetaFile) -> bool {
    dirty_at(&local_for(conn, rel), file)
}

pub(crate) fn dirty_at(local: &Path, file: &RemoteMetaFile) -> bool {
    file.downloaded
        && !file.is_dir
        && (file.local_stamp.is_empty()
            || stamp(local).ok().as_deref() != Some(file.local_stamp.as_str()))
        && (file.baseline.is_empty()
            || fingerprint(local)
                .map(|hash| hash != file.baseline)
                .unwrap_or(true))
}

pub async fn sync_one(path: &str, options: &TransferOptions) -> Result<(), String> {
    let Some((conn, roots)) = selected_connection(&[path.to_owned()])? else {
        return Ok(());
    };
    let _guard = lock(&conn).await;
    let (path, rel) = &roots[0];
    let mut meta = load_meta(&conn);
    validate_roots(&meta, &roots)?;
    sync_one_locked(path, rel, options, &conn, &mut meta)
}

fn sync_one_locked(
    path: &str,
    rel: &str,
    options: &TransferOptions,
    conn: &StoredConnection,
    meta: &mut RemoteMeta,
) -> Result<(), String> {
    let file = meta.files.entry(rel.to_owned()).or_default();
    if rel.is_empty() {
        file.is_dir = true;
    }
    file.pinned = true;
    file.excluded = false;
    let is_dir = file.is_dir;
    if is_dir {
        for (key, child) in &mut meta.files {
            if in_tree(key, rel) {
                child.pinned = true;
                child.excluded = false;
            }
        }
    }
    // Persist intent before starting I/O, so interrupted pins resume at startup.
    save_meta(conn, meta)?;
    let result = if is_dir {
        download_dir_inner(path, options)
    } else {
        download_inner(path, options).map(|_| ())
    };
    // Downloads persist their byte baseline independently; preserve that
    // committed metadata before the next item in a multi-selection starts.
    *meta = load_meta(conn);
    result
}

pub async fn sync(paths: &[String], options: &TransferOptions) -> Result<(), String> {
    let Some((conn, roots)) = selected_connection(paths)? else {
        return Ok(());
    };
    let _guard = lock(&conn).await;
    let mut meta = load_meta(&conn);
    validate_roots(&meta, &roots)?;
    for (path, rel) in roots {
        sync_one_locked(&path, &rel, options, &conn, &mut meta)?;
    }
    Ok(())
}

pub async fn unpin(paths: &[String]) -> Result<(), String> {
    let Some((conn, roots)) = selected_connection(paths)? else {
        return Ok(());
    };
    let _guard = lock(&conn).await;
    let mut meta = load_meta(&conn);
    validate_roots(&meta, &roots)?;
    for (_, rel) in roots {
        for (key, file) in meta.files.iter_mut() {
            if in_tree(key, &rel) && file.pinned {
                // Keep the bytes as a plain cache; the exclusion stops a pinned
                // ancestor from silently pinning it again.
                file.pinned = false;
                file.excluded = true;
            }
        }
    }
    save_meta(&conn, &meta)
}

pub async fn uncache(paths: &[String]) -> Result<(), String> {
    let Some((conn, roots)) = selected_connection(paths)? else {
        return Ok(());
    };
    let _guard = lock(&conn).await;
    let mut meta = load_meta(&conn);
    validate_roots(&meta, &roots)?;
    // Pins own their bytes; uncache only drops unpinned cache entries. Build
    // and validate the union before deleting any byte from a multi-selection.
    let keys: Vec<_> = meta
        .files
        .iter()
        .filter(|(key, file)| roots.iter().any(|(_, root)| in_tree(key, root)) && !file.pinned)
        .map(|(key, _)| key.clone())
        .collect();
    for key in &keys {
        let file = &meta.files[key];
        if file.conflict
            || (file.downloaded
                && !file.is_dir
                && local_for(&conn, key).is_file()
                && dirty(&conn, key, file))
        {
            return Err(crate::i18n::t("存在未同步的修改或冲突，不能取消缓存").into());
        }
    }
    for key in keys {
        let file = meta.files.get_mut(&key).unwrap();
        if !file.is_dir && local_for(&conn, &key).is_file() {
            fs::remove_file(local_for(&conn, &key)).map_err(|e| e.to_string())?;
        }
        file.downloaded = false;
        file.uploaded = false;
        file.baseline.clear();
        file.local_stamp.clear();
        file.excluded = true;
    }
    save_meta(&conn, &meta)
}

pub(crate) fn in_tree(key: &str, root: &str) -> bool {
    root.is_empty() || key == root || key.starts_with(&format!("{root}/"))
}
