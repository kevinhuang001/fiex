//! Generic plugin-owned virtual filesystems.
//!
//! A plugin may declare one or more path schemes in
//! `contributes.vfs.schemes`. Core never branches on a specific plugin ID:
//! it resolves the scheme to the installed, approved plugin and forwards
//! listing/stat/read/search requests over the same IPC used by every plugin.
use crate::{
    files::{Entry, Listing},
    plugin_host::{self, PluginInfo},
};
use serde_json::{json, Value};

const MAX_PAGES: usize = 1000;
const MAX_ENTRIES: usize = 100_000;

fn declared_schemes(plugin: &PluginInfo) -> Vec<&str> {
    let mut values = Vec::new();
    for key in ["connections", "toolbar"] {
        let Some(items) = plugin.contributions.get(key).and_then(Value::as_array) else {
            continue;
        };
        for item in items {
            if let Some(scheme) = item.get("scheme").and_then(Value::as_str) {
                values.push(scheme);
            }
            if let Some(schemes) = item.get("schemes").and_then(Value::as_array) {
                values.extend(schemes.iter().filter_map(Value::as_str));
            }
        }
    }
    values
        .into_iter()
        .filter(|scheme| {
            !scheme.is_empty()
                && scheme.len() <= 32
                && scheme
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'+')
        })
        .collect()
}

/// Split `scheme://rest` without accepting traversal or Windows separators.
pub fn split(path: &str) -> Option<(&str, &str)> {
    let (scheme, rest) = path.split_once("://")?;
    if scheme.is_empty()
        || scheme.len() > 32
        || !scheme
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'+')
    {
        return None;
    }
    if rest.contains('\0') || rest.split('/').any(|part| part == "." || part == "..") {
        return None;
    }
    Some((scheme, rest))
}

fn plugin_for(scheme: &str) -> Result<PluginInfo, String> {
    plugin_host::catalog()
        .into_iter()
        .find(|plugin| {
            plugin.enabled
                && plugin.approved
                && plugin.backend_available
                && declared_schemes(plugin).contains(&scheme)
        })
        .ok_or_else(|| format!("没有已安装并授权的插件提供 {scheme}:// 文件系统"))
}

pub fn handles(path: &str) -> bool {
    let Some((scheme, _)) = split(path) else {
        return false;
    };
    plugin_host::catalog().into_iter().any(|plugin| {
        plugin.enabled
            && plugin.approved
            && plugin.backend_available
            && declared_schemes(&plugin).contains(&scheme)
    })
}

fn invoke(plugin: &PluginInfo, method: &str, params: Value) -> Result<Value, String> {
    plugin_host::global().invoke(&plugin.id, method, params)
}

pub fn entry_from(value: Value) -> Result<Entry, String> {
    serde_json::from_value(value).map_err(|error| format!("插件返回了无效的目录项：{error}"))
}

pub fn list(path: &str, hidden: bool, ignores: &[String]) -> Result<Listing, String> {
    let (scheme, _) = split(path).ok_or("无效的插件文件系统路径")?;
    let plugin = plugin_for(scheme)?;
    let mut cursor: Option<String> = None;
    let mut entries = Vec::new();
    let mut warnings = Vec::new();
    for _ in 0..MAX_PAGES {
        let page = invoke(
            &plugin,
            "vfs.list",
            json!({
                "scheme": scheme,
                "nodeId": path,
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
        for item in items {
            entries.push(entry_from(item.clone())?);
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
        path: path.into(),
        entries,
        warnings,
    })
}

pub fn stat(path: &str) -> Result<Entry, String> {
    let (scheme, _) = split(path).ok_or("无效的插件文件系统路径")?;
    let plugin = plugin_for(scheme)?;
    let value = invoke(
        &plugin,
        "vfs.stat",
        json!({"scheme": scheme, "nodeId": path}),
    )?;
    entry_from(value)
}

pub fn read_small(path: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
    use base64::Engine;
    let (scheme, _) = split(path).ok_or("无效的插件文件系统路径")?;
    let plugin = plugin_for(scheme)?;
    let value = invoke(
        &plugin,
        "vfs.readSmall",
        json!({"scheme": scheme, "nodeId": path, "maxBytes": max_bytes.min(1024 * 1024)}),
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
    let (scheme, _) = split(path).ok_or("无效的插件文件系统路径")?;
    let plugin = plugin_for(scheme)?;
    let value = invoke(
        &plugin,
        "vfs.search",
        json!({
            "scheme": scheme,
            "nodeId": path,
            "query": query,
            "hidden": hidden,
            "ignores": ignores,
            "limit": limit
        }),
    )?;
    let entries = value
        .get("entries")
        .and_then(Value::as_array)
        .ok_or("插件搜索响应缺少 entries")?
        .iter()
        .cloned()
        .map(entry_from)
        .collect::<Result<Vec<_>, _>>()?;
    let truncated = value
        .get("truncated")
        .and_then(Value::as_bool)
        .unwrap_or(entries.len() >= limit);
    Ok((entries, truncated))
}
