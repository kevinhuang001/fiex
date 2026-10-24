//! Download bookkeeping plus sync, unsync and conflict helpers.

use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf};

use super::{
    config::{connection_for_path, local_for, rel_string},
    files::{download, download_dir, TransferOptions},
    store::{fiex_dir, StoredConnection},
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteMetaFile {
    pub(crate) size: u64,
    pub(crate) modified: u64,
    pub(crate) is_dir: bool,
    pub(crate) downloaded: bool,
}

#[derive(Default, Serialize, Deserialize)]
pub(crate) struct RemoteMeta {
    pub(crate) files: HashMap<String, RemoteMetaFile>,
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
    fs::write(
        meta_path(conn),
        serde_json::to_vec(meta).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

pub async fn sync_one(path: &str, options: &TransferOptions) -> Result<(), String> {
    let Some((conn, rel)) = connection_for_path(path) else {
        return Ok(());
    };
    let rel = rel_string(&rel);
    let meta = load_meta(&conn);
    if meta.files.get(&rel).is_some_and(|file| file.is_dir) {
        download_dir(path, options).await
    } else {
        download(path, options).await.map(|_| ())
    }
}

pub async fn sync(paths: &[String], options: &TransferOptions) -> Result<(), String> {
    for path in paths {
        sync_one(path, options).await?;
    }
    Ok(())
}

pub fn unsync(paths: &[String]) -> Result<(), String> {
    for path in paths {
        let Some((conn, rel)) = connection_for_path(path) else {
            continue;
        };
        let rel = rel_string(&rel);
        let mut meta = load_meta(&conn);
        let local = local_for(&conn, &rel);
        if local.is_dir() {
            let prefix = format!("{rel}/");
            let keys = meta
                .files
                .iter()
                .filter(|(key, _)| key.as_str() == rel || key.starts_with(&prefix))
                .map(|(key, _)| key.clone())
                .collect::<Vec<_>>();
            for key in keys {
                let child = local_for(&conn, &key);
                if meta.files.get(&key).is_some_and(|file| !file.is_dir) {
                    let _ = fs::remove_file(&child);
                }
                if let Some(file) = meta.files.get_mut(&key) {
                    file.downloaded = false;
                }
            }
        } else {
            let _ = fs::remove_file(&local);
            if let Some(file) = meta.files.get_mut(&rel) {
                file.downloaded = false;
            }
        }
        save_meta(&conn, &meta)?;
    }
    Ok(())
}
