//! Generic plugin-owned virtual filesystems.
//!
//! Explicit VFS contributions are routed through Core-owned view identities.
//! Connections use the same URI resolver and the existing remote I/O adapter.
//! UI contributions never confer provider eligibility.
use crate::files::{Entry, Listing};
use serde_json::{json, Value};

const MAX_PAGES: usize = 1000;
const MAX_ENTRIES: usize = 100_000;

/// Claim every Core URI, including invalid/revoked ones, before OS fallback.
pub fn handles(path: &str) -> bool {
    crate::mounts::handles(path)
}

fn resolve(path: &str) -> Result<(crate::mounts::Uri, crate::mounts::View), String> {
    let (uri, source) = crate::mounts::resolve(path)?;
    match source {
        crate::mounts::Source::View(view) => Ok((uri, view)),
        crate::mounts::Source::Connection(_) => {
            Err("connection request requires the Core connection adapter".into())
        }
    }
}
fn node(path: &str) -> Result<(crate::mounts::Uri, crate::mounts::View, String), String> {
    let (uri, mut view) = resolve(path)?;
    if view.node_id(&uri.relative)?.is_none() {
        // Address-bar/bookmark navigation may target an opaque node not visited
        // this session. Resolve only its ancestors, never enumerate the mount.
        let mut parent = String::new();
        for part in uri.relative.split('/') {
            let target = if parent.is_empty() {
                part.to_string()
            } else {
                format!("{parent}/{part}")
            };
            let current = crate::mounts::view(&uri.mount_id)?;
            if current.node_id(&target)?.is_none() {
                list(&crate::mounts::uri(&uri.mount_id, &parent)?, true, &[])?;
            }
            parent = target;
        }
        view = crate::mounts::view(&uri.mount_id)?;
    }
    let id = view.node_id(&uri.relative)?.ok_or("VFS node not found")?;
    Ok((uri, view, id))
}
fn invoke(view: &crate::mounts::View, method: &str, mut params: Value) -> Result<Value, String> {
    params["contributionId"] = json!(view.descriptor.contribution_id);
    params["viewContext"] = view.descriptor.view_context.clone();
    crate::plugin_host::global().invoke_vfs(
        &view.descriptor.plugin_id,
        method,
        params,
        view.scope.clone(),
    )
}
fn project_entry(
    mut value: Value,
    uri: &crate::mounts::Uri,
    relative: &str,
) -> Result<(Entry, String), String> {
    let id = value
        .get("nodeId")
        .and_then(Value::as_str)
        .ok_or("provider entry missing nodeId")?
        .to_string();
    if id.len() > 16384 || id.contains('\0') {
        return Err("invalid provider node ID".into());
    }
    // A provider's self-claimed path is never an authority or a required ID.
    value["path"] = json!("");
    let mut entry = entry_from(value)?;
    crate::mounts::segment(&entry.name)?;
    if !relative.is_empty() && relative.rsplit('/').next() != Some(entry.name.as_str()) {
        return Err("provider node name does not match its location".into());
    }
    entry.path = crate::mounts::uri(&uri.mount_id, relative)?;
    entry.readonly = true;
    entry.sync = None;
    Ok((entry, id))
}
fn bound_entry(value: Value, uri: &crate::mounts::Uri, relative: &str) -> Result<Entry, String> {
    let (entry, id) = project_entry(value, uri, relative)?;
    crate::mounts::bind_node(&uri.mount_id, relative, &id)?;
    Ok(entry)
}
pub fn entry_from(value: Value) -> Result<Entry, String> {
    serde_json::from_value(value).map_err(|error| format!("插件返回了无效的目录项：{error}"))
}

pub fn list(path: &str, hidden: bool, ignores: &[String]) -> Result<Listing, String> {
    let (uri, view, node_id) = node(path)?;
    let mut cursor: Option<String> = None;
    let mut entries = Vec::new();
    let mut warnings = Vec::new();
    for _ in 0..MAX_PAGES {
        let page = invoke(
            &view,
            "vfs.list",
            json!({
                "nodeId": node_id,
                "hidden": hidden,
                "ignores": ignores,
                "cursor": cursor.clone(),
                "limit": 500
            }),
        )?;
        let items = page
            .get("entries")
            .and_then(Value::as_array)
            .ok_or("插件目录响应缺少 entries")?;
        if items.len() > 500 {
            return Err("provider exceeded page limit".into());
        }
        for item in items {
            let name = item["name"].as_str().ok_or("provider entry missing name")?;
            crate::mounts::segment(name)?;
            let relative = if uri.relative.is_empty() {
                name.to_string()
            } else {
                format!("{}/{name}", uri.relative)
            };
            entries.push(bound_entry(item.clone(), &uri, &relative)?);
            if entries.len() > MAX_ENTRIES {
                return Err("插件目录项超过上限".into());
            }
        }
        if let Some(values) = page.get("warnings").and_then(Value::as_array) {
            warnings.extend(values.iter().filter_map(Value::as_str).map(str::to_owned));
        }
        let next = page
            .get("nextCursor")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if next.is_none() {
            cursor = None;
            break;
        }
        if next == cursor {
            return Err("插件分页游标没有前进".into());
        }
        cursor = next;
    }
    if cursor.is_some() {
        return Err("插件目录分页过多".into());
    }
    Ok(Listing {
        read_only: true,
        virtual_volume: true,
        path: crate::mounts::uri(&uri.mount_id, &uri.relative)?,
        entries,
        warnings,
    })
}

pub fn stat(path: &str) -> Result<Entry, String> {
    let (uri, view, node_id) = node(path)?;
    let value = invoke(&view, "vfs.stat", json!({ "nodeId": node_id}))?;
    bound_entry(value, &uri, &uri.relative)
}

pub fn read_small(path: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
    use base64::Engine;
    let (_uri, view, node_id) = node(path)?;
    let value = invoke(
        &view,
        "vfs.readSmall",
        json!({ "nodeId": node_id, "maxBytes": max_bytes.min(1024 * 1024)}),
    )?;
    let encoded = value
        .get("base64")
        .and_then(Value::as_str)
        .ok_or("插件预览响应缺少数据")?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| "插件返回了无效的 Base64 数据")?;
    if bytes.len() > max_bytes {
        return Err("插件预览数据超过请求上限".into());
    }
    Ok(bytes)
}

pub fn search(
    path: &str,
    query: &str,
    hidden: bool,
    ignores: &[String],
    limit: usize,
) -> Result<(Vec<Entry>, bool), String> {
    let (uri, view, node_id) = node(path)?;
    let value = invoke(
        &view,
        "vfs.search",
        json!({
            "nodeId": node_id,
            "query": query,
            "hidden": hidden,
            "ignores": ignores,
            "limit": limit
        }),
    )?;
    let items = value
        .get("entries")
        .and_then(Value::as_array)
        .ok_or("插件搜索响应缺少 entries")?;
    if items.len() > limit {
        return Err("provider exceeded search limit".into());
    }
    let entries = value
        .get("entries")
        .and_then(Value::as_array)
        .ok_or("插件搜索响应缺少 entries")?
        .iter()
        .cloned()
        .map(|value| {
            let relative = value["relativePath"]
                .as_str()
                .ok_or("search entry missing relativePath")?
                .to_string();
            crate::mounts::uri(&uri.mount_id, &relative)?;
            if !uri.relative.is_empty() && !relative.starts_with(&format!("{}/", uri.relative)) {
                return Err("search entry escaped subtree".into());
            }
            bound_entry(value, &uri, &relative)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let truncated = value
        .get("truncated")
        .and_then(Value::as_bool)
        .unwrap_or(entries.len() >= limit);
    Ok((entries, truncated))
}

/// Read-only, bounded exports for external applications. No tree snapshots or
/// persistent blob cache: temp directories live until process exit, independently
/// of tabs, so closing one tab cannot invalidate another consumer's export.
fn exports() -> &'static std::sync::Mutex<std::collections::HashMap<String, tempfile::TempDir>> {
    static EXPORTS: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, tempfile::TempDir>>,
    > = std::sync::OnceLock::new();
    EXPORTS.get_or_init(Default::default)
}
pub fn cleanup_exports() {
    if let Ok(mut exports) = exports().lock() {
        #[cfg(windows)]
        for directory in exports.values() {
            if let Ok(entries) = std::fs::read_dir(directory.path()) {
                for entry in entries.flatten() {
                    if let Ok(metadata) = entry.metadata() {
                        let mut permissions = metadata.permissions();
                        permissions.set_readonly(false);
                        let _ = std::fs::set_permissions(entry.path(), permissions);
                    }
                }
            }
        }
        exports.clear();
    }
}
pub fn materialize(path: &str) -> Result<String, String> {
    let entry = stat(path)?; // rechecks the owner and scope even on export reuse
    if entry.is_dir {
        return Err("VFS directories cannot be opened as OS folders".into());
    }
    const MAX_EXPORT: usize = 1024 * 1024;
    if entry.size > MAX_EXPORT as u64 {
        return Err("此 VFS 的有界读取接口不支持导出超过 1 MiB 的文件".into());
    }
    crate::mounts::segment(&entry.name)?;
    let mut exports = exports()
        .lock()
        .map_err(|_| "export registry unavailable")?;
    if let Some(directory) = exports.get(path) {
        return Ok(directory
            .path()
            .join(&entry.name)
            .to_string_lossy()
            .into_owned());
    }
    if exports.len() >= 64 {
        return Err("too many open VFS exports; restart Fiex to release temporary exports".into());
    }
    let bytes = read_small(path, MAX_EXPORT)?;
    if bytes.len() as u64 != entry.size {
        return Err("VFS export is incomplete".into());
    }
    let directory = tempfile::tempdir().map_err(|e| e.to_string())?;
    let target = directory.path().join(&entry.name);
    std::fs::write(&target, bytes).map_err(|e| e.to_string())?;
    let mut permissions = std::fs::metadata(&target)
        .map_err(|e| e.to_string())?
        .permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&target, permissions).map_err(|e| e.to_string())?;
    let result = target.to_string_lossy().into_owned();
    exports.insert(path.into(), directory);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn provider_cannot_choose_a_different_mount_or_escape_with_a_name() {
        let location = crate::mounts::parse("fiex://mount/owner-one/folder").unwrap();
        let value = json!({"nodeId":"opaque-node","path":"fiex://mount/foreign/secret", "name":"report #%.txt", "isDir":false,"isSymlink":false,"size":7,"modified":0,"created":0,"extension":"txt","mime":"text/plain","readonly":false,"linkTarget":null,"tags":[],"sync":"pinned"});
        let (entry, node) =
            project_entry(value.clone(), &location, "folder/report #%.txt").unwrap();
        assert_eq!(node, "opaque-node");
        assert_eq!(
            entry.path,
            "fiex://mount/owner-one/folder/report%20%23%25.txt"
        );
        assert!(entry.readonly && entry.sync.is_none());
        let mut unsafe_value = value.clone();
        unsafe_value["name"] = json!("../secret");
        assert!(project_entry(unsafe_value, &location, "folder/../secret").is_err());
        assert!(project_entry(value, &location, "other.txt").is_err());
    }
}
