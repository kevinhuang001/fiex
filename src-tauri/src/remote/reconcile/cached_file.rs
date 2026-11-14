//! Reconcile one materialized cache entry against an injected provider boundary.

use super::super::{
    cache::{CacheChange, cache_change},
    config::{check_cache_path, local_for},
    files::{TransferOptions, fetch_remote},
    store::StoredConnection,
    sync::{RemoteMeta, RemoteMetaFile, dirty_at, fingerprint, load_meta, save_meta, stamp},
};
use serde_json::{Value, json};
use std::{fs, path::Path};

pub(super) struct ReconcileOutcome {
    pub(super) result: Result<(), String>,
    pub(super) metadata_changed: bool,
}

pub(super) struct ReconcileContext<'a> {
    pub(super) conn: &'a StoredConnection,
    pub(super) rel: &'a str,
    pub(super) pull: bool,
    pub(super) changes: &'a mut Vec<CacheChange>,
    pub(super) meta: &'a mut RemoteMeta,
    pub(super) local: &'a Path,
    pub(super) file: RemoteMetaFile,
}

pub(super) struct ReconcileIo<S, F, U> {
    pub(super) stat: S,
    pub(super) fetch: F,
    pub(super) upload: U,
}

pub(super) fn reconcile_file(
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
    let outcome = reconcile_cached_file_with(
        ReconcileContext {
            conn,
            rel,
            pull,
            changes,
            meta: &mut meta,
            local: &local,
            file,
        },
        ReconcileIo {
            stat,
            fetch: |conn: &StoredConnection, node: &str, destination: &Path| {
                fetch_remote(conn, node, destination, &TransferOptions::default()).map(|_| ())
            },
            upload: super::upload::upload,
        },
    );
    if outcome.metadata_changed {
        save_meta(conn, &meta)?;
    }
    outcome.result
}

pub(super) fn stat(
    conn: &StoredConnection,
    rel: &str,
    file: &RemoteMetaFile,
) -> Result<Value, String> {
    super::invoke(
        conn,
        "vfs.stat",
        json!({"nodeId": if file.node_id.is_empty() { rel } else { &file.node_id }}),
    )
}

pub(super) fn reconcile_cached_file_with<S, F, U>(
    context: ReconcileContext<'_>,
    io: ReconcileIo<S, F, U>,
) -> ReconcileOutcome
where
    S: FnMut(&StoredConnection, &str, &RemoteMetaFile) -> Result<Value, String>,
    F: FnMut(&StoredConnection, &str, &Path) -> Result<(), String>,
    U: FnMut(&StoredConnection, &str, &RemoteMetaFile) -> Result<String, String>,
{
    let ReconcileContext {
        conn,
        rel,
        pull,
        changes,
        meta,
        local,
        file,
    } = context;
    let ReconcileIo {
        mut stat,
        mut fetch,
        mut upload,
    } = io;
    if file.is_dir || !file.downloaded || file.conflict {
        return ReconcileOutcome {
            result: Ok(()),
            metadata_changed: false,
        };
    }
    let mut metadata_changed = false;
    let result = (|| {
        if !local.is_file() {
            if pull {
                return Err(crate::i18n::t("缓存文件缺失").into());
            }
            // A vanished unpinned cache is forgotten; the next open downloads it.
            let entry = meta.files.get_mut(rel).ok_or("cache entry disappeared")?;
            entry.downloaded = false;
            entry.uploaded = false;
            entry.baseline.clear();
            entry.local_stamp.clear();
            metadata_changed = true;
            changes.push(cache_change(conn, rel, "uncached"));
            return Ok(());
        }
        let changed = dirty_at(local, &file);
        let remote = stat(conn, rel, &file)?;
        let revision = remote["revision"]
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or("provider has no revision support")?;
        if revision != file.revision {
            if changed {
                meta.files
                    .get_mut(rel)
                    .ok_or("cache entry disappeared")?
                    .conflict = true;
                metadata_changed = true;
                changes.push(cache_change(conn, rel, "conflict"));
                return Err(crate::i18n::t("本地与云端均有修改，请处理同步冲突").into());
            }
            if !pull {
                // Cache is push-only: a newer remote revision invalidates the copy
                // instead of being fetched behind the user's back.
                fs::remove_file(local).map_err(|error| error.to_string())?;
                let entry = meta.files.get_mut(rel).ok_or("cache entry disappeared")?;
                entry.downloaded = false;
                entry.uploaded = false;
                entry.baseline.clear();
                entry.local_stamp.clear();
                entry.revision = revision.into();
                metadata_changed = true;
                changes.push(cache_change(conn, rel, "uncached"));
                return Ok(());
            }
            // Download to a separate sibling and recheck local contents before
            // replacement, so an editor save during network I/O is preserved.
            let staged =
                tempfile::NamedTempFile::new_in(local.parent().ok_or("invalid cache path")?)
                    .map_err(|error| error.to_string())?;
            fetch(conn, &file.node_id, staged.path())?;
            if fingerprint(local)? != file.baseline {
                return Err(crate::i18n::t("本地文件在下载期间改变，稍后重试").into());
            }
            let after = stat(conn, rel, &file)?;
            if after["revision"].as_str() != Some(revision) {
                return Err(crate::i18n::t("云端文件正在变化，稍后重试").into());
            }
            staged
                .persist(local)
                .map_err(|error| error.error.to_string())?;
            let entry = meta.files.get_mut(rel).ok_or("cache entry disappeared")?;
            entry.revision = revision.into();
            entry.baseline = fingerprint(local)?;
            entry.local_stamp = stamp(local)?;
            entry.size = fs::metadata(local)
                .map_err(|error| error.to_string())?
                .len();
            entry.uploaded = false;
            metadata_changed = true;
            changes.push(cache_change(conn, rel, "cached"));
        } else if changed {
            upload(conn, rel, &file)?;
            changes.push(cache_change(conn, rel, "uploaded"));
        }
        Ok(())
    })();
    ReconcileOutcome {
        result,
        metadata_changed,
    }
}
