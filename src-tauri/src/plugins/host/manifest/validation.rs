use super::*;
use fiex_provider_contracts::{InfoCompute, InfoFieldDescriptor, InfoSectionDescriptor};
use std::collections::HashSet;
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
            return Err(
                "extensions.info.command is removed; info.compute is fixed and fields are declared in sections"
                    .into(),
            );
        }
        if key == "info" {
            if object.keys().any(|field| {
                !matches!(
                    field.as_str(),
                    "match" | "priority" | "sections" | "providers"
                )
            }) {
                return Err(
                    "extensions.info accepts only match, priority, sections and providers".into(),
                );
            }
            if let Some(priority) = object.get("priority") {
                let priority = priority
                    .as_i64()
                    .ok_or("extensions.info.priority must be an integer")?;
                if !(-100_000..=100_000).contains(&priority) {
                    return Err("extensions.info.priority is out of range".into());
                }
            }
        }
        crate::plugins::match_rules::validate(object.get("match"), false, false)?;
        if key == "info" {
            validate_info_registration(object)?;
        }
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
        if key == "hooks" {
            if object.keys().any(|field| {
                !matches!(field.as_str(), "event" | "match" | "priority" | "providers")
            }) {
                return Err("extensions.hooks accepts event, match, priority and providers".into());
            }
            if !matches!(
                object.get("event").and_then(Value::as_str),
                Some("beforeOpen" | "afterOpen")
            ) {
                return Err("extensions.hooks.event must be beforeOpen or afterOpen".into());
            }
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
            crate::plugins::match_rules::validate(object.get("match"), false, false)?;
            connections::validate(object)?;
        }

        if object.contains_key("readOnly") {
            return Err(format!(
                "extensions.{key}.readOnly is removed; declare VFS operations"
            ));
        }
    }
    Ok(())
}

fn validate_info_registration(object: &serde_json::Map<String, Value>) -> Result<(), String> {
    if let Some(sections) = object.get("sections") {
        validate_info_sections(sections)?;
    }
    if let Some(providers) = object.get("providers").and_then(Value::as_array) {
        for provider in providers {
            let provider = provider
                .as_object()
                .ok_or("extensions.info.providers entries must be objects")?;
            let sections = provider
                .get("sections")
                .or_else(|| object.get("sections"))
                .ok_or("each Info provider must declare non-empty sections")?;
            validate_info_sections(sections)?;
        }
    } else if object.get("sections").is_none() {
        return Err("extensions.info.sections must declare non-empty sections".into());
    }
    Ok(())
}

fn validate_info_sections(value: &Value) -> Result<(), String> {
    const MAX_INFO_FIELDS: usize = 64;
    let sections = value
        .as_array()
        .filter(|sections| !sections.is_empty() && sections.len() <= 64)
        .ok_or("Info sections must contain 1..64 entries")?;
    let mut section_ids = HashSet::new();
    let mut field_count = 0_usize;
    for value in sections {
        let section = value
            .as_object()
            .ok_or("Info sections must contain objects")?;
        if section
            .keys()
            .any(|key| !matches!(key.as_str(), "id" | "title" | "fields"))
        {
            return Err("Info section accepts only id, title and fields".into());
        }
        let id = section
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let title = section
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !valid_info_id(id)
            || id.len() > 128
            || title.trim().is_empty()
            || title.len() > 1024
            || !section_ids.insert(id)
        {
            return Err("Info section has an invalid or duplicate id/title".into());
        }
        let fields = section
            .get("fields")
            .and_then(Value::as_array)
            .filter(|fields| !fields.is_empty() && fields.len() <= MAX_INFO_FIELDS)
            .ok_or("Info section fields must contain 1..64 entries")?;
        field_count = field_count.saturating_add(fields.len());
        if field_count > MAX_INFO_FIELDS {
            return Err("Info provider may declare at most 64 fields".into());
        }
        let mut field_ids = HashSet::new();
        for value in fields {
            let field = value
                .as_object()
                .ok_or("Info fields must contain objects")?;
            if field
                .keys()
                .any(|key| !matches!(key.as_str(), "id" | "label" | "compute"))
            {
                return Err("Info fields accept only id, label and compute".into());
            }
            let descriptor: InfoFieldDescriptor = serde_json::from_value(value.clone())
                .map_err(|error| format!("invalid Info field descriptor: {error}"))?;
            if !valid_info_id(&descriptor.id)
                || descriptor.id.len() > 128
                || descriptor.label.trim().is_empty()
                || descriptor.label.len() > 1024
                || !field_ids.insert(descriptor.id)
                || !matches!(
                    descriptor.compute,
                    InfoCompute::OnOpen | InfoCompute::OnDemand
                )
            {
                return Err("Info field has an invalid or duplicate id/label/compute".into());
            }
        }
    }
    let _: Vec<InfoSectionDescriptor> = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid Info section descriptor: {error}"))?;
    Ok(())
}

fn valid_info_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
}
