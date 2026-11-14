use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use super::super::store::{StoredConnection, fiex_dir};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteMetaFile {
    pub(crate) size: u64,
    pub(crate) modified: u64,
    pub(crate) is_dir: bool,
    pub(crate) downloaded: bool,
    pub(crate) node_id: String,
    pub(crate) revision: String,
    /// Provider `allowedOps` for this node; `None` means unrestricted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) allowed_ops: Option<Vec<String>>,
    pub(crate) baseline: String,
    pub(crate) local_stamp: String,
    pub(crate) pinned: bool,
    pub(crate) excluded: bool,
    pub(crate) conflict: bool,
    /// The last local edit was accepted by the remote.
    pub(crate) uploaded: bool,
}

#[derive(Default, Serialize, Deserialize)]
pub(crate) struct RemoteMeta {
    pub(crate) files: HashMap<String, RemoteMetaFile>,
    pub(crate) listed: HashSet<String>,
    /// Direct child paths from the last complete listing of each directory.
    /// This keeps offline snapshots from reviving entries removed remotely.
    pub(crate) listed_children: HashMap<String, Vec<String>>,
    pub(crate) errors: HashMap<String, String>,
}

fn meta_path(conn: &StoredConnection) -> PathBuf {
    fiex_dir()
        .join("remote-meta")
        .join(format!("{}.json", conn.id))
}

/// Deleting a connection must not leave the cached tree or its metadata
/// behind; the next connection that reuses the ID starts clean.
pub(crate) fn forget_meta(conn: &StoredConnection) {
    let _ = fs::remove_file(meta_path(conn));
}

pub(crate) fn load_meta(conn: &StoredConnection) -> RemoteMeta {
    load_meta_from(&meta_path(conn))
}

pub(crate) fn load_meta_from(path: &Path) -> RemoteMeta {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub(crate) fn save_meta(conn: &StoredConnection, meta: &RemoteMeta) -> Result<(), String> {
    save_meta_to(&meta_path(conn), meta)
}

pub(crate) fn save_meta_to(path: &Path, meta: &RemoteMeta) -> Result<(), String> {
    let parent = path.parent().ok_or("invalid remote metadata path")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    use std::io::Write;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temp.write_all(&serde_json::to_vec(meta).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(path).map_err(|e| e.error.to_string())?;
    Ok(())
}
