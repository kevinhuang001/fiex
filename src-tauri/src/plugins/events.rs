//! Fixed Core-owned system events. Custom plugin RPC methods intentionally
//! bypass this router through the separately authorized plugin.invoke bridge.
use serde_json::Value;
use std::path::PathBuf;

pub struct RegisteredProvider {
    pub plugin: crate::plugin_host::PluginInfo,
    pub contribution: Value,
    pub priority: i64,
}

fn extension_for(method: &str) -> Option<&'static str> {
    match method {
        "info.resolve" => Some("info"),
        "preview.open" => Some("preview"),
        "decorations.resolve" => Some("decorations"),
        "contextMenu.resolve" | "contextMenu.execute" => Some("contextMenus"),
        "search.query" => Some("search"),
        "toolbar.invoke" => Some("toolbar"),
        "hooks.invoke" => Some("hooks"),
        "connection.open" | "connection.create" | "connection.validate" => Some("connections"),
        _ => None,
    }
}

pub fn matching_plugins(
    method: &str,
    ext: &str,
    mime: &str,
    is_dir: bool,
) -> Vec<RegisteredProvider> {
    let Some(extension) = extension_for(method) else {
        return Vec::new();
    };
    let mut providers = Vec::new();
    for plugin in crate::plugin_host::catalog() {
        if !plugin.enabled || !plugin.approved || !plugin.backend_available {
            continue;
        }
        let Some(contribution) = plugin.extensions.get(extension) else {
            continue;
        };
        if contribution.is_null() || contribution.as_bool() == Some(false) {
            continue;
        }
        let rules = crate::plugin_match::MatchRules::from_contribution(contribution);
        if !rules.matches(ext, mime, is_dir) {
            continue;
        }
        let priority = contribution
            .get("priority")
            .and_then(Value::as_i64)
            .unwrap_or(50);
        providers.push(RegisteredProvider {
            plugin,
            contribution: contribution.clone(),
            priority,
        });
    }
    providers.sort_by_key(|provider| std::cmp::Reverse(provider.priority));
    providers
}

/// Core-owned system events are not exposed through the plugin UI bridge.
/// Unknown methods remain legal plugin-local business methods.
pub fn is_protected_method(method: &str) -> bool {
    extension_for(method).is_some()
        || [
            "host.",
            "core.",
            "plugin.",
            "contextMenu.",
            "vfs.",
            "fs.",
            "connection.",
            "preview.",
            "info.",
            "decorations.",
            "search.",
            "toolbar.",
            "hooks.",
            "terminal.",
        ]
        .iter()
        .any(|prefix| method.starts_with(prefix))
}

fn require_event(method: &str) -> Result<(), String> {
    if extension_for(method).is_some() {
        Ok(())
    } else {
        Err(format!("unregistered Core event: {method}"))
    }
}

pub fn invoke(plugin_id: &str, method: &str, params: Value) -> Result<Value, String> {
    require_event(method)?;
    crate::plugin_host::global().invoke(plugin_id, method, params)
}

pub fn invoke_at(
    plugin_id: &str,
    method: &str,
    params: Value,
    directory: &str,
) -> Result<Value, String> {
    require_event(method)?;
    crate::plugin_host::global().invoke_at(plugin_id, method, params, directory)
}

pub fn invoke_selected(
    plugin_id: &str,
    method: &str,
    params: Value,
    directory: &str,
    paths: &[PathBuf],
) -> Result<Value, String> {
    require_event(method)?;
    crate::plugin_host::global().invoke_selected(plugin_id, method, params, directory, paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_events_are_core_owned_custom_methods_remain_available() {
        for method in [
            "info.resolve",
            "preview.open",
            "contextMenu.resolve",
            "search.query",
            "toolbar.invoke",
            "hooks.invoke",
            "host.fs.openRead",
        ] {
            assert!(is_protected_method(method), "{method}");
        }
        for method in ["checksum.calculate", "git.stage", "archive.compress"] {
            assert!(!is_protected_method(method), "{method}");
        }
    }
}
