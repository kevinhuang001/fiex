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
    /// The provider marked this node read-only regardless of its capabilities.
    pub(crate) readonly: bool,
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
/// UI projection of a node's capabilities. `readonly` is only the provider's
/// explicit node flag; the operation set is the authoritative per-action gate,
/// so a node that supports just `remove` stays deletable.
pub(super) fn entry_capabilities(
    plugin_id: &str,
    allowed: Option<&[String]>,
    explicit_readonly: bool,
) -> (bool, Vec<String>) {
    let operations = effective_ops(plugin_id, allowed);
    (
        explicit_readonly,
        operations
            .into_iter()
            .map(|operation| operation.as_str().to_string())
            .collect(),
    )
}

pub(super) fn parse_allowed_ops(entry: &serde_json::Value) -> Option<Vec<String>> {
    entry
        .get("allowedOps")
        .and_then(serde_json::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(serde_json::Value::as_str)
                .filter(|name| FsOperation::parse(name).is_some())
                .map(str::to_owned)
                .collect()
        })
}

pub(crate) fn list_remote(conn: &StoredConnection, rel: &str) -> Result<Vec<RemoteFile>, String> {
    let plugin_id = ensure_provider_access(conn)?;
    let config = provider_config(conn);
    let metadata = load_meta(conn);
    let node_id = metadata
        .files
        .get(rel)
        .map(|m| m.node_id.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(rel);
    let mut files = Vec::new();
    let mut cursor: Option<String> = None;
    let mut page_count = 0usize;
    loop {
        page_count += 1;
        if page_count > 1000 {
            return Err("provider sent too many pages".into());
        }
        let page = crate::plugins::host::global().invoke(
            &plugin_id,
            "fs.list",
            serde_json::json!({
                "mountId": conn.mount_id,
                "connection": config,
                "nodeId": node_id,
                "cursor": cursor.as_deref().unwrap_or("0"),
                "limit": 500
            }),
        )?;
        let entries = page
            .get("entries")
            .and_then(serde_json::Value::as_array)
            .ok_or("provider response missing entries")?;
        if entries.len() > 500 || files.len() + entries.len() > 100_000 {
            return Err("provider exceeded listing limit".into());
        }
        for entry in entries {
            let name = entry
                .get("name")
                .and_then(serde_json::Value::as_str)
                .ok_or("provider entry missing name")?;
            if safe_child_name(name).is_none() {
                return Err("provider returned an unsafe child name".into());
            }
            let kind = entry
                .get("kind")
                .and_then(serde_json::Value::as_str)
                .ok_or("provider entry missing kind")?;
            if kind != "directory" && kind != "file" {
                return Err("provider returned an unknown node kind".into());
            }
            let node_id = entry
                .get("nodeId")
                .and_then(serde_json::Value::as_str)
                .ok_or("provider entry missing nodeId")?;
            files.push(RemoteFile {
                node_id: node_id.into(),
                revision: entry
                    .get("revision")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .into(),
                allowed_ops: parse_allowed_ops(entry),
                readonly: entry
                    .get("readonly")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
                name: name.into(),
                is_dir: kind == "directory",
                size: entry
                    .get("size")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or_default(),
                modified: entry
                    .get("modified")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or_default(),
            });
        }
        let next = page
            .get("nextCursor")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        if next.is_none() {
            break;
        }
        if next == cursor || files.len() > 100_000 {
            return Err("provider pagination is invalid or too large".into());
        }
        cursor = next;
    }
    Ok(files)
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
    let listed = tokio::task::spawn_blocking(move || list_remote(&listing_conn, &listing_rel))
        .await
        .map_err(|error| error.to_string())?;
    let files = match listed {
        Ok(files) => {
            remember_listing(&rel, &files, &mut metadata);
            save_meta(&conn, &metadata)?;
            files
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
    let cached_dirs = super::super::cache::cached_dirs(&metadata);
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
        let state = metadata.files.get(&child);
        let downloaded = state.is_some_and(|meta| meta.downloaded) && local.is_file();
        let has_cache = if file.is_dir {
            cached_dirs.contains(&child)
        } else {
            downloaded
        };
        let pinned = state.is_some_and(|meta| meta.pinned);
        let dirty = state.is_some_and(|meta| super::super::sync::dirty(&conn, &child, meta));
        // "Uploaded" is orthogonal to the sync mode, but only meaningful while
        // the entry still has a cache or a pin (an uncached entry has no copy).
        let uploaded = state.is_some_and(|meta| meta.uploaded && !meta.conflict)
            && !dirty
            && (has_cache || pinned);
        let (readonly, allowed_ops) =
            entry_capabilities(&plugin_id, file.allowed_ops.as_deref(), file.readonly);
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
            readonly,
            link_target: None,
            allowed_ops,
            tags: Vec::new(),
            has_cache,
            pinned,
            uploaded,
            sync: Some(
                state
                    .map(|m| {
                        if m.conflict {
                            "conflict"
                        } else if dirty {
                            "pending"
                        } else if m.pinned {
                            "pinned"
                        } else if has_cache {
                            "synced"
                        } else {
                            "cloud"
                        }
                    })
                    .unwrap_or("cloud")
                    .into(),
            ),
        });
    }
    entries.sort_by_cached_key(|entry| (!entry.is_dir, entry.name.to_lowercase()));
    // A mount is not inherently read-only: the provider declaration and each
    // node's `allowedOps` decide which actions the UI may offer.
    // Creating a folder needs the provider's `mkdir` and the directory node's
    // own stored `allowedOps`/`readonly`. `for_path` is the same rule
    // `create_entry` enforces, so the toolbar's new-folder affordance and the
    // command cannot disagree.
    let can_create = crate::plugins::resource::file_capabilities::for_path(path).create;
    Ok(Some(crate::core::files::Listing {
        read_only: false,
        can_create,
        virtual_volume: true,
        path: super::super::config::navigation_path(&conn, &rel)?,
        entries,
        warnings,
    }))
}
