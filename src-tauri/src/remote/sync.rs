//! Download bookkeeping plus pin, uncache and conflict helpers.

use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf};

use super::{
    config::{connection_for_path, local_for, rel_string},
    files::{download_dir_inner, download_inner, TransferOptions},
    store::{fiex_dir, StoredConnection},
};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteMetaFile {
    pub(crate) size: u64,
    pub(crate) modified: u64,
    pub(crate) is_dir: bool,
    pub(crate) downloaded: bool,
    #[serde(default)]
    pub(crate) node_id: String,
    #[serde(default)]
    pub(crate) revision: String,
    #[serde(default)]
    pub(crate) baseline: String,
    #[serde(default)]
    pub(crate) local_stamp: String,
    #[serde(default)]
    pub(crate) pinned: bool,
    #[serde(default)]
    pub(crate) excluded: bool,
    #[serde(default)]
    pub(crate) conflict: bool,
    /// The last local edit was accepted by the remote.
    #[serde(default)]
    pub(crate) uploaded: bool,
}

#[derive(Default, Serialize, Deserialize)]
pub(crate) struct RemoteMeta {
    pub(crate) files: HashMap<String, RemoteMetaFile>,
    #[serde(default)]
    pub(crate) listed: std::collections::HashSet<String>,
    #[serde(default)]
    pub(crate) errors: HashMap<String, String>,
}

fn meta_dir() -> PathBuf {
    fiex_dir().join("remote-meta")
}

fn meta_path(conn: &StoredConnection) -> PathBuf {
    meta_dir().join(format!("{}.json", conn.id))
}

pub(crate) fn load_meta(conn: &StoredConnection) -> RemoteMeta {
    fs::read(meta_path(conn))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub(crate) fn save_meta(conn: &StoredConnection, meta: &RemoteMeta) -> Result<(), String> {
    fs::create_dir_all(meta_dir()).map_err(|e| e.to_string())?;
    use std::io::Write;
    let mut temp = tempfile::NamedTempFile::new_in(meta_dir()).map_err(|e| e.to_string())?;
    temp.write_all(&serde_json::to_vec(meta).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(meta_path(conn))
        .map_err(|e| e.error.to_string())?;
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
    file.downloaded
        && !file.is_dir
        && (file.local_stamp.is_empty()
            || stamp(&local_for(conn, rel)).ok().as_deref() != Some(file.local_stamp.as_str()))
        && (file.baseline.is_empty()
            || fingerprint(&local_for(conn, rel))
                .map(|hash| hash != file.baseline)
                .unwrap_or(true))
}

pub async fn sync_one(path: &str, options: &TransferOptions) -> Result<(), String> {
    let Some((conn, rel)) = connection_for_path(path) else {
        return Ok(());
    };
    let _guard = lock(&conn).await;
    let rel = rel_string(&rel);
    let mut meta = load_meta(&conn);
    let file = meta.files.get_mut(&rel).ok_or("请先刷新远程目录")?;
    file.pinned = true;
    file.excluded = false;
    let is_dir = file.is_dir;
    if is_dir {
        for (key, child) in &mut meta.files {
            if in_tree(key, &rel) {
                child.pinned = true;
                child.excluded = false;
            }
        }
    }
    // Persist intent before starting I/O, so interrupted pins resume at startup.
    save_meta(&conn, &meta)?;
    if is_dir {
        download_dir_inner(path, options)
    } else {
        download_inner(path, options).map(|_| ())
    }
}

pub async fn sync(paths: &[String], options: &TransferOptions) -> Result<(), String> {
    for path in paths {
        sync_one(path, options).await?;
    }
    Ok(())
}

pub async fn unpin(paths: &[String]) -> Result<(), String> {
    for path in paths {
        let Some((conn, rel)) = connection_for_path(path) else {
            continue;
        };
        let _guard = lock(&conn).await;
        let rel = rel_string(&rel);
        let mut meta = load_meta(&conn);
        for (key, file) in meta.files.iter_mut() {
            if in_tree(key, &rel) && file.pinned {
                // Keep the bytes as a plain cache; the exclusion stops a pinned
                // ancestor from silently pinning it again.
                file.pinned = false;
                file.excluded = true;
            }
        }
        save_meta(&conn, &meta)?;
    }
    Ok(())
}

pub async fn uncache(paths: &[String]) -> Result<(), String> {
    for path in paths {
        let Some((conn, rel)) = connection_for_path(path) else {
            continue;
        };
        let _guard = lock(&conn).await;
        let rel = rel_string(&rel);
        let mut meta = load_meta(&conn);
        // Pins own their bytes; uncache only drops unpinned cache entries.
        let keys: Vec<_> = meta
            .files
            .iter()
            .filter(|(key, file)| in_tree(key, &rel) && !file.pinned)
            .map(|(key, _)| key.clone())
            .collect();
        // Validate the whole selection before removing any bytes.
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
        save_meta(&conn, &meta)?;
    }
    Ok(())
}

pub(crate) fn in_tree(key: &str, root: &str) -> bool {
    root.is_empty() || key == root || key.starts_with(&format!("{root}/"))
}
