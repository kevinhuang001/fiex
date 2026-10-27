//! Cache and pin state helpers shared by listings, reconciliation and menus.

use serde::Serialize;

use super::{
    config::navigation_path,
    store::StoredConnection,
    sync::{in_tree, RemoteMeta},
};

/// A cache or pin transition detected by background reconciliation. The
/// desktop shell forwards these to the activity centre.
#[derive(Clone, Serialize)]
pub struct CacheChange {
    pub path: String,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

pub(crate) fn cache_change(conn: &StoredConnection, rel: &str, state: &str) -> CacheChange {
    CacheChange {
        path: navigation_path(conn, rel).unwrap_or_else(|_| rel.to_string()),
        state: state.into(),
        detail: None,
    }
}

/// A failed sync step; the activity centre shows the path and the reason.
pub(crate) fn cache_failure(conn: &StoredConnection, rel: &str, error: &str) -> CacheChange {
    CacheChange {
        path: navigation_path(conn, rel).unwrap_or_else(|_| rel.to_string()),
        state: "failed".into(),
        detail: Some(error.to_string()),
    }
}

/// Directories that hold at least one downloaded file, so listings can show a
/// directory as cached before it is opened.
pub(crate) fn cached_dirs(meta: &RemoteMeta) -> std::collections::HashSet<String> {
    let mut dirs = std::collections::HashSet::new();
    for (key, file) in &meta.files {
        if !file.downloaded {
            continue;
        }
        let mut parent = key.rsplit_once('/').map(|(parent, _)| parent).unwrap_or("");
        while !parent.is_empty() && dirs.insert(parent.to_string()) {
            parent = parent
                .rsplit_once('/')
                .map(|(parent, _)| parent)
                .unwrap_or("");
        }
    }
    dirs
}

pub(crate) fn subtree_has_cache(meta: &RemoteMeta, rel: &str) -> bool {
    meta.files
        .iter()
        .any(|(key, file)| file.downloaded && in_tree(key, rel))
}

/// Cache entries with bytes on disk, used to diff one reconciliation round.
pub(crate) fn downloaded_keys(meta: &RemoteMeta) -> std::collections::HashSet<String> {
    meta.files
        .iter()
        .filter(|(_, file)| file.downloaded)
        .map(|(key, _)| key.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote::sync::RemoteMetaFile;

    fn cached(is_dir: bool) -> RemoteMetaFile {
        RemoteMetaFile {
            downloaded: true,
            is_dir,
            ..Default::default()
        }
    }

    #[test]
    fn cached_dirs_cover_every_ancestor() {
        let mut meta = RemoteMeta::default();
        meta.files.insert("a".into(), RemoteMetaFile::default());
        meta.files.insert("a/b".into(), RemoteMetaFile::default());
        meta.files.insert("a/b/c.txt".into(), cached(false));
        let dirs = cached_dirs(&meta);
        assert!(dirs.contains("a"));
        assert!(dirs.contains("a/b"));
        assert!(!dirs.contains(""));
        assert!(subtree_has_cache(&meta, "a"));
        assert!(!subtree_has_cache(&meta, "z"));
    }

    #[test]
    fn downloads_are_not_pins() {
        let meta: RemoteMeta = serde_json::from_str(
            r#"{"files":{"a":{"size":1,"modified":0,"isDir":false,"downloaded":true}}}"#,
        )
        .unwrap();
        assert!(meta.files["a"].downloaded);
        assert!(!meta.files["a"].pinned);
        assert!(cached_dirs(&meta).is_empty());
    }
}
