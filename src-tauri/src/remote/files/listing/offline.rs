use super::*;

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
    let cache_states = super::super::super::cache::states_for(&conn, &metadata, &nodes);
    let mut warnings = vec![crate::i18n::format(
        "离线目录：{error}",
        &[("error", error.to_string())],
    )];
    warnings.extend(
        metadata
            .errors
            .iter()
            .filter(|(key, _)| super::super::super::sync::in_tree(key, &rel))
            .map(|(key, detail)| format!("{key}: {detail}")),
    );
    let mut entries = Vec::with_capacity(files.len());
    for (file, child) in files.into_iter().zip(children) {
        let local = local_for(&conn, &child);
        let state = cache_states[&child];
        entries.push(crate::core::files::Entry {
            path: super::super::super::config::navigation_path(&conn, &child)?,
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
mod tests {
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
