use super::*;

#[derive(Clone)]
pub(crate) struct RemoteFile {
    pub(crate) name: String,
    pub(crate) is_dir: bool,
    pub(crate) size: u64,
    pub(crate) modified: u64,
    pub(crate) node_id: String,
    pub(crate) revision: String,
    /// Node-level restriction from the provider (`allowedOps`); `None` means
    /// the node did not narrow the plugin's declared capabilities.
    pub(crate) allowed_ops: Option<Vec<String>>,
}

pub(crate) struct RemoteListing {
    pub(crate) files: Vec<RemoteFile>,
    pub(crate) complete: bool,
    pub(crate) warnings: Vec<String>,
}
/// Capabilities a mount node really exposes: the plugin's declaration
/// intersected with the entry's own `allowedOps`. Core never grants more than
/// either layer allows.
pub(super) fn effective_ops(plugin_id: &str, allowed: Option<&[String]>) -> Vec<FsOperation> {
    let declared = crate::plugins::provider_registry::operations(plugin_id).unwrap_or_default();
    FsOperation::ALL
        .into_iter()
        .filter(|operation| declared.contains(operation))
        .filter(|operation| {
            allowed.is_none_or(|list| {
                list.iter()
                    .any(|name| FsOperation::parse(name) == Some(*operation))
            })
        })
        .collect()
}
/// Operations a node exposes after intersecting provider and node declarations.
pub(super) fn entry_capabilities(plugin_id: &str, allowed: Option<&[String]>) -> Vec<String> {
    effective_ops(plugin_id, allowed)
        .into_iter()
        .map(|operation| operation.as_str().to_string())
        .collect()
}

pub(crate) fn list_remote(conn: &StoredConnection, rel: &str) -> Result<RemoteListing, String> {
    let plugin_id = ensure_provider_access(conn)?;
    let metadata = load_meta(conn);
    let node_id = metadata
        .files
        .get(rel)
        .map(|m| m.node_id.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(rel);
    let mount = crate::plugins::providers::fs::Mount {
        mount_id: conn.mount_id.clone(),
        plugin_id,
        source: crate::plugins::providers::fs::Source::Connection(provider_config(conn)),
        scope: None,
    };
    let listing = mount.list(node_id, true, &[])?;
    let files = listing
        .entries
        .into_iter()
        .map(|entry| {
            if safe_child_name(&entry.name).is_none() {
                return Err("provider returned an unsafe child name".to_string());
            }
            let is_dir = entry.is_dir();
            Ok(RemoteFile {
                name: entry.name,
                is_dir,
                size: entry.size.unwrap_or_default(),
                modified: entry.modified.unwrap_or_default().max(0) as u64,
                node_id: entry.node_id,
                revision: entry.revision.unwrap_or_default(),
                allowed_ops: entry.allowed_ops.map(|ops| {
                    ops.into_iter()
                        .map(|operation| operation.as_str().to_owned())
                        .collect()
                }),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RemoteListing {
        files,
        complete: !listing.truncated,
        warnings: listing.warnings,
    })
}

/// Directory listing rendered from the plugin's VFS metadata. A virtual
/// remote item is represented by a stable logical mount path, not an empty
/// file on disk. Materialization happens only when the user opens/downloads.
pub async fn list_listing(
    path: &str,
    hidden: bool,
    ignores: &[String],
) -> Result<Option<crate::core::files::Listing>, String> {
    let Some((conn, relative)) = connection_for_path(path) else {
        return Ok(None);
    };
    let plugin_id = ensure_provider_access(&conn)?;
    let _guard = super::super::sync::lock(&conn).await;
    let rel = rel_string(&relative);
    let mut metadata = load_meta(&conn);
    let mut warnings = Vec::new();
    // Provider RPC and remote pagination are synchronous. Keep them off the
    // async runtime while preserving the per-connection metadata lock.
    let listing_conn = conn.clone();
    let listing_rel = rel.clone();
    let listed = if tokio::runtime::Handle::try_current().is_ok() {
        tokio::task::spawn_blocking(move || list_remote(&listing_conn, &listing_rel))
            .await
            .map_err(|error| error.to_string())?
    } else {
        // Headless integration clients can drive this async API with a plain
        // executor. They have no Tokio reactor; never panic by spawning onto
        // a runtime that does not exist.
        list_remote(&listing_conn, &listing_rel)
    };
    let files = match listed {
        Ok(listing) => {
            remember_listing(&rel, &listing.files, &mut metadata, listing.complete);
            save_meta(&conn, &metadata)?;
            if !listing.complete {
                warnings
                    .push(crate::i18n::t("目录列表不完整；不能用于离线浏览或整目录缓存").into());
            }
            warnings.extend(listing.warnings);
            listing.files
        }
        Err(error) => {
            if !metadata.listed.contains(&rel) {
                return Err(error);
            }
            warnings.push(crate::i18n::format(
                "离线目录：{error}",
                &[("error", error.to_string())],
            ));
            cached_listing(&rel, &metadata)
        }
    };
    warnings.extend(
        metadata
            .errors
            .iter()
            .filter(|(key, _)| super::super::sync::in_tree(key, &rel))
            .map(|(key, error)| format!("{key}: {error}")),
    );
    let cached_dirs = super::super::cache::cached_dirs(&conn, &metadata);
    let filters = crate::core::ignores::IgnoreSet::new(ignores);
    let mut entries = Vec::new();
    for file in files {
        if (!hidden && file.name.starts_with('.')) || filters.is_ignored(Path::new(&file.name)) {
            continue;
        }
        let child = if relative.as_os_str().is_empty() {
            file.name.clone()
        } else {
            format!("{}/{}", rel_string(&relative), file.name)
        };
        let local = local_for(&conn, &child);
        let cache_state = super::super::cache::state_with_cached_dirs(
            &conn,
            &metadata,
            &child,
            file.is_dir,
            &cached_dirs,
        );
        let has_cache = cache_state.has_cache;
        let pinned = cache_state.pinned;
        let dirty = cache_state.dirty;
        // "Uploaded" is orthogonal to the sync mode, but only meaningful while
        // the entry still has a cache or a pin (an uncached entry has no copy).
        let uploaded = cache_state.uploaded;
        let allowed_ops = entry_capabilities(&plugin_id, file.allowed_ops.as_deref());
        entries.push(crate::core::files::Entry {
            path: super::super::config::navigation_path(&conn, &child)?,
            name: file.name,
            is_dir: file.is_dir,
            is_symlink: false,
            size: file.size,
            modified: file.modified,
            created: 0,
            extension: local
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase(),
            mime: mime_guess::from_path(&local)
                .first_or_octet_stream()
                .to_string(),
            link_target: None,
            allowed_ops,
            tags: Vec::new(),
            has_cache,
            pinned,
            uploaded,
            storage_projection: Default::default(),
            sync: Some(
                if cache_state.conflict {
                    "conflict"
                } else if dirty {
                    "pending"
                } else if pinned {
                    "pinned"
                } else if cache_state.cached {
                    "synced"
                } else if cache_state.partial {
                    "partial"
                } else {
                    "cloud"
                }
                .into(),
            ),
        });
    }
    entries.sort_by_cached_key(|entry| (!entry.is_dir, entry.name.to_lowercase()));
    let allowed_ops =
        crate::plugins::resource::file_capabilities::for_path(path).into_allowed_ops();
    Ok(Some(crate::core::files::Listing {
        allowed_ops,
        virtual_volume: true,
        path: super::super::config::navigation_path(&conn, &rel)?,
        entries,
        warnings,
        default_view: None,
    }))
}

/// Return a previously complete Connection listing after a provider call
/// fails. This revalidates the current connection/provider authorization
/// before exposing metadata and never falls back for standalone Views.
type OfflineListing = (Vec<crate::core::files::Entry>, Vec<String>);

pub(crate) fn offline_provider_listing(
    path: &str,
    error: &str,
) -> Result<Option<OfflineListing>, String> {
    if !is_offline_failure(error) {
        return Ok(None);
    }
    let Some((conn, relative)) = connection_for_path(path) else {
        return Ok(None);
    };
    let plugin_id = ensure_provider_access(&conn)?;
    let rel = rel_string(&relative);
    let metadata = load_meta(&conn);
    if !metadata.listed.contains(&rel) {
        return Ok(None);
    }
    let files = cached_listing(&rel, &metadata);
    let children = files
        .iter()
        .map(|file| {
            if rel.is_empty() {
                file.name.clone()
            } else {
                format!("{rel}/{}", file.name)
            }
        })
        .collect::<Vec<_>>();
    let nodes = files
        .iter()
        .zip(&children)
        .map(|(file, child)| (child.as_str(), file.is_dir))
        .collect::<Vec<_>>();
    let cache_states = super::super::cache::states_for(&conn, &metadata, &nodes);
    let mut warnings = vec![crate::i18n::format(
        "离线目录：{error}",
        &[("error", error.to_string())],
    )];
    warnings.extend(
        metadata
            .errors
            .iter()
            .filter(|(key, _)| super::super::sync::in_tree(key, &rel))
            .map(|(key, detail)| format!("{key}: {detail}")),
    );
    let mut entries = Vec::with_capacity(files.len());
    for (file, child) in files.into_iter().zip(children) {
        let local = local_for(&conn, &child);
        let state = cache_states[&child];
        entries.push(crate::core::files::Entry {
            path: super::super::config::navigation_path(&conn, &child)?,
            name: file.name,
            is_dir: file.is_dir,
            is_symlink: false,
            size: file.size,
            modified: file.modified,
            created: 0,
            extension: local
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase(),
            mime: mime_guess::from_path(&local)
                .first_or_octet_stream()
                .to_string(),
            link_target: None,
            allowed_ops: entry_capabilities(&plugin_id, file.allowed_ops.as_deref()),
            tags: Vec::new(),
            has_cache: state.has_cache,
            pinned: state.pinned,
            uploaded: state.uploaded,
            storage_projection: Default::default(),
            sync: Some(
                if state.conflict {
                    "conflict"
                } else if state.dirty {
                    "pending"
                } else if state.pinned {
                    "pinned"
                } else if state.cached {
                    "synced"
                } else if state.partial {
                    "partial"
                } else {
                    "cloud"
                }
                .into(),
            ),
        });
    }
    entries.sort_by_cached_key(|entry| (!entry.is_dir, entry.name.to_lowercase()));
    Ok(Some((entries, warnings)))
}

fn is_offline_failure(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    if [
        "permissiondenied",
        "permission denied",
        "access denied",
        "forbidden",
        "unsupported:",
        "notfound:",
        "scope denied",
        "stale ref",
        "conflict:",
        "revoked",
        "unauthorized",
    ]
    .iter()
    .any(|marker| error.contains(marker))
    {
        return false;
    }
    [
        "offline",
        "offline:",
        "network error",
        "network is unreachable",
        "no route to host",
        "connection refused",
        "connection reset",
        "connection timed out",
        "timed out",
        "timeout",
        "failed to resolve",
        "name or service not known",
        "temporary failure in name resolution",
        "temporarily unavailable",
    ]
    .iter()
    .any(|marker| error.contains(marker))
}

#[cfg(test)]
mod offline_failure_tests {
    use super::is_offline_failure;

    #[test]
    fn cached_listing_fallback_is_limited_to_network_failures() {
        assert!(is_offline_failure("Offline: service unavailable"));
        assert!(is_offline_failure("connection timed out"));
        assert!(!is_offline_failure("PermissionDenied: access denied"));
        assert!(!is_offline_failure("Unsupported: vfs.list"));
        assert!(!is_offline_failure("NotFound: directory removed"));
    }
}
