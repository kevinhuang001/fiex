use super::*;
use std::path::{Component, Path};

pub fn valid_id(id: &str) -> bool {
    id.len() > 2
        && id.len() < 128
        && id.contains('.')
        && !id.starts_with('.')
        && !id.contains("..")
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

/// A declared package entry must be a relative slash-separated path inside the
/// plugin package. URLs, absolute paths, drive letters, parent segments and
/// backslashes are never resolved by Core; reads also re-check canonical
/// containment so a symlink cannot escape the package.
pub(crate) fn valid_package_entry(entry: &str) -> bool {
    if entry.is_empty()
        || entry.len() > 1024
        || entry.contains("://")
        || entry.contains(':')
        || entry.contains('\\')
        || entry.contains('\0')
    {
        return false;
    }
    let path = Path::new(entry);
    path.is_relative()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

/// Optional `extensions.preview.loading.entry`, already validated by
/// [`validate_extensions`] when the manifest is discovered.
pub(crate) fn preview_loading_entry(extensions: &Value) -> Option<&str> {
    extensions
        .get("preview")
        .and_then(Value::as_object)
        .and_then(|preview| preview.get("loading"))
        .and_then(Value::as_object)
        .and_then(|loading| loading.get("entry"))
        .and_then(Value::as_str)
}

/// Extensions declare provider eligibility, never resource permissions.
pub fn validate_extensions(value: &Value) -> Result<(), String> {
    if !value.is_null() && !value.is_object() {
        return Err("extensions must be an object".into());
    }
    for feature in [
        "info",
        "preview",
        "decorations",
        "contextMenus",
        "search",
        "hooks",
        "toolbar",
        "connectionContextMenus",
    ] {
        if let Some(object) = value.get(feature).and_then(Value::as_object) {
            super::provider_instances::validate(feature, object)?;
        }
    }
    if let Some(enabled) = value.get("contextMenus") {
        match enabled {
            Value::Bool(_) => {}
            Value::Object(object) => {
                crate::plugins::match_rules::validate(object.get("match"), false, true)?
            }
            _ => return Err("extensions.contextMenus must be true or {match}".into()),
        }
    }
    if let Some(enabled) = value.get("connectionContextMenus") {
        match enabled {
            Value::Bool(_) => {}
            Value::Object(object) => {
                if let Some(priority) = object.get("priority") {
                    if !priority.is_i64() && !priority.is_u64() {
                        return Err(
                            "extensions.connectionContextMenus.priority must be an integer".into(),
                        );
                    }
                }
                crate::plugins::match_rules::validate(object.get("match"), false, false)?;
                if object
                    .keys()
                    .any(|key| !matches!(key.as_str(), "priority" | "match" | "providers"))
                {
                    return Err(
                        "extensions.connectionContextMenus accepts priority, match and providers"
                            .into(),
                    );
                }
            }
            _ => return Err("extensions.connectionContextMenus must be true or {priority}".into()),
        }
    }
    if let Some(decorations) = value.get("decorations") {
        match decorations {
            Value::Bool(_) => {}
            Value::Object(object) => {
                for forbidden in ["mode", "method", "scope", "rules"] {
                    if object.contains_key(forbidden) {
                        return Err(format!(
                            "extensions.decorations.{forbidden} is removed; decorations are dynamic and use decorations.resolve"
                        ));
                    }
                }
                crate::plugins::match_rules::validate(object.get("match"), true, false)?;
            }
            _ => return Err("extensions.decorations must be true or {match,priority}".into()),
        }
    }
    for key in ["info", "preview"] {
        let Some(item) = value.get(key) else {
            continue;
        };
        let object = item
            .as_object()
            .ok_or_else(|| format!("extensions.{key} must be an object"))?;
        if object.contains_key("title") {
            return Err(format!(
                "extensions.{key}.title is removed; the plugin name is used"
            ));
        }
        if key == "info" && object.contains_key("id") {
            return Err("extensions.info.id is not used; remove it".into());
        }
        if key == "info" && object.contains_key("command") {
            return Err("extensions.info.command is removed; info.resolve is fixed".into());
        }
        crate::plugins::match_rules::validate(object.get("match"), false, false)?;
        if key == "preview" {
            if let Some(loading) = object.get("loading") {
                let loading = loading
                    .as_object()
                    .ok_or("extensions.preview.loading must be an object")?;
                for field in loading.keys() {
                    if field != "entry" {
                        return Err(format!(
                            "extensions.preview.loading.{field} is not supported"
                        ));
                    }
                }
                let entry = loading
                    .get("entry")
                    .and_then(Value::as_str)
                    .ok_or("extensions.preview.loading.entry is required")?;
                if !valid_package_entry(entry) {
                    return Err(
                        "extensions.preview.loading.entry must be a package-relative path".into(),
                    );
                }
            }
        }
    }
    for key in ["search", "hooks"] {
        let Some(item) = value.get(key) else {
            continue;
        };
        let object = item
            .as_object()
            .ok_or_else(|| format!("extensions.{key} must be an object"))?;
        if object.contains_key("id") {
            return Err(format!(
                "extensions.{key}.id is not used; the plugin id is the contribution id"
            ));
        }
        if key == "search" && (object.contains_key("title") || object.contains_key("command")) {
            return Err(
                "extensions.search.title/command are removed; search.query is fixed".into(),
            );
        }
        crate::plugins::match_rules::validate(object.get("match"), false, false)?;
        if key == "hooks" && object.contains_key("method") {
            return Err("extensions.hooks.method is removed; hooks.invoke is fixed".into());
        }
    }
    if let Some(themes) = value.get("themes") {
        let object = themes
            .as_object()
            .ok_or("extensions.themes must be an object")?;
        if object.contains_key("id") || object.contains_key("title") {
            return Err(
                "extensions.themes.id/title are not used; the theme file carries them".into(),
            );
        }
        if object
            .get("path")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        {
            return Err("extensions.themes.path is required".into());
        }
    }
    if let Some(ui) = value.get("ui") {
        let object = ui.as_object().ok_or("extensions.ui must be an object")?;
        for key in object.keys() {
            if key != "timeoutMs" {
                return Err(format!("extensions.ui.{key} is not supported"));
            }
        }
        if let Some(timeout) = object.get("timeoutMs") {
            let timeout = timeout
                .as_u64()
                .ok_or("extensions.ui.timeoutMs must be a positive integer")?;
            if !(MIN_UI_TIMEOUT_MS..=MAX_UI_TIMEOUT_MS).contains(&timeout) {
                return Err(format!(
                    "extensions.ui.timeoutMs must be between {MIN_UI_TIMEOUT_MS} and {MAX_UI_TIMEOUT_MS}"
                ));
            }
        }
    }
    for key in ["vfs", "connections", "toolbar"] {
        let Some(item) = value.get(key) else {
            continue;
        };
        let object = item
            .as_object()
            .ok_or_else(|| format!("extensions.{key} must be an object"))?;
        if object.contains_key("id") {
            return Err(format!(
                "extensions.{key}.id is not used; the plugin id is the contribution id"
            ));
        }
        for forbidden in [
            "scheme",
            "schemes",
            "uriPrefix",
            "mountId",
            "mountPath",
            "capabilities",
        ] {
            if object.contains_key(forbidden) {
                return Err(format!(
                    "extensions.{key}.{forbidden} is forbidden; Core owns VFS addresses"
                ));
            }
        }
        if object
            .get("title")
            .and_then(Value::as_str)
            .is_none_or(|s| s.trim().is_empty() || s.len() > 256)
        {
            return Err(format!("extensions.{key}.title is required"));
        }
        if key == "toolbar" && object.contains_key("command") {
            return Err("extensions.toolbar.command is removed; toolbar.invoke is fixed".into());
        }
        if key == "connections" && !object.get("fields").is_some_and(Value::is_array) {
            return Err("extensions.connections.fields must be an array".into());
        }
        if key == "connections" {
            connections::validate(object)?;
        }

        if object.get("readOnly").is_some_and(|v| !v.is_boolean()) {
            return Err(format!("extensions.{key}.readOnly must be a boolean"));
        }
    }
    Ok(())
}
