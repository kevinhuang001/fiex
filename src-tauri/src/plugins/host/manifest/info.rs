use super::*;
use fiex_provider_contracts::{InfoCompute, InfoFieldDescriptor, InfoProviderPresentation};
use std::collections::HashSet;

pub(super) fn validate_registration(object: &serde_json::Map<String, Value>) -> Result<(), String> {
    if let Some(providers) = object.get("providers").and_then(Value::as_array) {
        for provider in providers {
            let provider = provider
                .as_object()
                .ok_or("extensions.info.providers entries must be objects")?;
            let title = provider
                .get("title")
                .ok_or("each named Info provider must declare its own title and subsections")?;
            let subsections = provider
                .get("subsections")
                .ok_or("each named Info provider must declare its own title and subsections")?;
            validate_info_presentation(title, subsections)?;
        }
        if object.contains_key("title") || object.contains_key("subsections") {
            return Err("extensions.info cannot share a presentation with named providers".into());
        }
    } else {
        let title = object
            .get("title")
            .ok_or("extensions.info.title is required")?;
        let subsections = object
            .get("subsections")
            .ok_or("extensions.info.subsections must be declared")?;
        validate_info_presentation(title, subsections)?;
    }
    Ok(())
}

fn validate_info_presentation(title: &Value, value: &Value) -> Result<(), String> {
    const MAX_INFO_FIELDS: usize = 64;
    let title = title.as_str().ok_or("Info provider title must be text")?;
    if title.trim().is_empty() || title.len() > 256 {
        return Err("Info provider title must contain 1..256 bytes".into());
    }
    let subsections = value
        .as_array()
        .filter(|subsections| !subsections.is_empty() && subsections.len() <= 64)
        .ok_or("Info subsections must contain 1..64 entries")?;
    let mut subsection_ids = HashSet::new();
    let mut field_count = 0_usize;
    for value in subsections {
        let subsection = value
            .as_object()
            .ok_or("Info subsections must contain objects")?;
        if subsection
            .keys()
            .any(|key| !matches!(key.as_str(), "id" | "match" | "fields"))
        {
            return Err("Info subsection accepts only id, match and fields".into());
        }
        let id = subsection
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !valid_info_id(id) || id.len() > 128 || !subsection_ids.insert(id) {
            return Err("Info subsection has an invalid or duplicate id".into());
        }
        validate_info_subsection_match(subsection.get("match"))?;
        let fields = subsection
            .get("fields")
            .and_then(Value::as_array)
            .filter(|fields| !fields.is_empty() && fields.len() <= MAX_INFO_FIELDS)
            .ok_or("Info subsection fields must contain 1..64 entries")?;
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
    let _: InfoProviderPresentation = serde_json::from_value(serde_json::json!({
        "title": title,
        "subsections": value,
    }))
    .map_err(|error| format!("invalid Info provider presentation: {error}"))?;
    Ok(())
}

fn validate_info_subsection_match(value: Option<&Value>) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    let object = value
        .as_object()
        .ok_or("Info subsection match must be an object")?;
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "extensions" | "mimeTypes" | "kinds"))
    {
        return Err("Info subsection match accepts only extensions, mimeTypes and kinds".into());
    }
    let string_list = |key: &str, max: usize| -> Result<Vec<String>, String> {
        let Some(value) = object.get(key) else {
            return Ok(Vec::new());
        };
        let values = value
            .as_array()
            .filter(|values| values.len() <= max)
            .ok_or_else(|| format!("Info subsection match {key} must be a bounded array"))?;
        values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| format!("Info subsection match {key} values must be text"))
            })
            .collect()
    };
    let extensions = string_list("extensions", 64)?;
    if has_duplicate_info_match_values(&extensions, |item| {
        item.trim_start_matches('.').to_ascii_lowercase()
    }) {
        return Err("Info subsection match contains duplicate extensions".into());
    }
    if extensions.iter().any(|value| {
        let normalized = value.trim_start_matches('.');
        normalized.is_empty()
            || !normalized.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'+')
            })
    }) {
        return Err("Info subsection match contains an invalid extension".into());
    }
    let mime_types = string_list("mimeTypes", 64)?;
    if has_duplicate_info_match_values(&mime_types, str::to_ascii_lowercase) {
        return Err("Info subsection match contains duplicate MIME types".into());
    }
    if mime_types
        .iter()
        .any(|value| !valid_info_mime_pattern(value))
    {
        return Err("Info subsection match contains an invalid MIME type".into());
    }
    let kinds = string_list("kinds", 8)?;
    if has_duplicate_info_match_values(&kinds, str::to_ascii_lowercase) {
        return Err("Info subsection match contains duplicate kinds".into());
    }
    if kinds
        .iter()
        .any(|kind| !matches!(kind.as_str(), "file" | "directory" | "symlink"))
    {
        return Err("Info subsection match kinds accept file, directory or symlink".into());
    }
    Ok(())
}

fn has_duplicate_info_match_values(values: &[String], normalize: impl Fn(&str) -> String) -> bool {
    let mut seen = HashSet::new();
    values.iter().any(|value| !seen.insert(normalize(value)))
}

fn valid_info_mime_pattern(value: &str) -> bool {
    let Some((kind, subtype)) = value.split_once('/') else {
        return false;
    };
    !kind.is_empty()
        && !subtype.is_empty()
        && value.matches('/').count() == 1
        && kind
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        && (subtype == "*"
            || subtype.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'+')
            }))
}

fn valid_info_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
}
