//! Uploading local (or materialized) trees into a provider-backed connection.
//!
//! The provider receives ordinary `vfs.beginWrite` calls: `parentId` + `name`
//! for new nodes and `nodeId` + `ifRevision` for replacements. Core keeps the
//! metadata snapshot in step so the explorer shows the new entries without a
//! full re-list.

use super::{
    config::{connection_for_path, ensure_provider_access, navigation_path, rel_string},
    files::{child_path, list_remote, RemoteFile},
    store::StoredConnection,
    sync::{load_meta, save_meta, RemoteMetaFile},
};
use crate::{
    core::{
        ignores::IgnoreSet,
        operations::{check_cancelled, OperationResult},
        transfers::{Action, Conflict, FileVersion, Resolution},
    },
    plugins::resolvers::fs::{mount_for_path, FsEntry, Mount, WriteRequest},
};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

mod planning;
use planning::walk;

mod execution;
use execution::copy_children;
pub(crate) use execution::upload;

mod streams;
use streams::{remember, upload_file};

const CHUNK: usize = 256 * 1024;

/// One top-level item to upload. `local` is a real file after remote/VFS
/// sources have been materialized by the caller.
pub(crate) struct Source {
    pub(crate) local: PathBuf,
    pub(crate) name: String,
}

struct FileTask {
    local: PathBuf,
    rel: String,
    name: String,
    size: u64,
    replace: Option<RemoteFile>,
}

struct DirTask {
    rel: String,
    name: String,
    /// Present when a same-named file must be removed before mkdir.
    existed: Option<RemoteFile>,
    /// Present when an existing directory is merged into.
    node: Option<String>,
}

fn unique_name(name: &str, taken: &HashSet<String>) -> String {
    if !taken.contains(name) {
        return name.to_string();
    }
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.to_string());
    let ext = path
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();
    for index in 2..10_000 {
        let candidate = format!("{stem} ({index}){ext}");
        if !taken.contains(&candidate) {
            return candidate;
        }
    }
    name.to_string()
}

fn version(local: &Path) -> Result<(u64, u64, bool), String> {
    let meta = fs::symlink_metadata(local).map_err(|error| error.to_string())?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    Ok((meta.len(), modified, meta.is_dir()))
}

fn conflict_key(local: &Path, destination: &str, source_size: u64, dest_size: u64) -> String {
    format!(
        "{}\0{}\0{}\0{}",
        local.to_string_lossy(),
        destination,
        source_size,
        dest_size
    )
}

fn action_for(key: &str, resolutions: &[Resolution], keep_both: bool) -> Option<Action> {
    if keep_both {
        return Some(Action::KeepBoth);
    }
    resolutions
        .iter()
        .find(|resolution| resolution.key == key)
        .map(|resolution| resolution.action)
}
