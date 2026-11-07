use serde_json::Value;

fn valid_info_id(value: &str) -> bool {
    value.is_ascii()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
}

pub fn validate_result(method: &str, value: &Value) -> Result<(), String> {
    if serde_json::to_vec(value).map_err(|e| e.to_string())?.len() > 4 * 1024 * 1024 {
        return Err("provider result exceeds limit".into());
    }
    let array = |key: &str, limit: usize| {
        value
            .get(key)
            .and_then(Value::as_array)
            .filter(|a| a.len() <= limit)
            .ok_or_else(|| format!("invalid {key} result"))
    };
    match method {
        "info.describe" => {
            let sections = array("sections", 64)?;
            let decoded = serde_json::from_value::<
                Vec<fiex_provider_contracts::InfoSectionDescriptor>,
            >(Value::Array(sections.clone()))
            .map_err(|error| format!("invalid Info descriptor: {error}"))?;
            let mut section_ids = std::collections::HashSet::new();
            for section in decoded {
                if !valid_info_id(&section.id)
                    || section.id.len() > 128
                    || section.title.is_empty()
                    || section.title.len() > 1024
                    || !section_ids.insert(section.id.clone())
                    || section.fields.len() > 256
                {
                    return Err("invalid or duplicate Info section".into());
                }
                let mut field_ids = std::collections::HashSet::new();
                for field in section.fields {
                    if !valid_info_id(&field.id)
                        || field.id.len() > 128
                        || field.label.is_empty()
                        || field.label.len() > 1024
                        || !field_ids.insert(field.id)
                    {
                        return Err("invalid or duplicate Info field".into());
                    }
                }
            }
        }
        "info.compute" => {
            let fields = array("fields", 64)?;
            let decoded = serde_json::from_value::<Vec<fiex_provider_contracts::InfoFieldResult>>(
                Value::Array(fields.clone()),
            )
            .map_err(|error| format!("invalid Info field results: {error}"))?;
            let mut ids = std::collections::HashSet::new();
            for field in decoded {
                if !valid_info_id(&field.section_id)
                    || field.section_id.len() > 128
                    || !valid_info_id(&field.field_id)
                    || field.field_id.len() > 128
                    || !ids.insert((field.section_id, field.field_id))
                    || field
                        .value
                        .as_ref()
                        .is_some_and(|value| value.len() > 16_384)
                    || field.unavailable && field.value.is_some()
                    || !field.unavailable && field.value.is_none()
                {
                    return Err("invalid or duplicate Info field result".into());
                }
            }
        }
        "preview.open" => {
            if !matches!(
                value["kind"].as_str(),
                Some("html" | "text" | "image" | "media" | "pdf" | "fallback" | "unsupported")
            ) {
                return Err("invalid preview kind".into());
            }
        }
        "contextMenu.resolve" => {
            array("items", 100)?;
        }
        "connectionContextMenu.resolve" => {
            array("items", 100)?;
            for item in value["items"].as_array().into_iter().flatten() {
                if item.get("parentId").is_some() {
                    return Err("connection menu items must be flat".into());
                }
            }
        }
        "decorations.resolve" => {
            array("decorations", 100_000)?;
        }
        "search.query" => {
            array("entries", 500)?;
        }
        "hooks.invoke" => {
            serde_json::from_value::<fiex_provider_contracts::HookResult>(value.clone())
                .map_err(|error| format!("invalid hook response: {error}"))?;
        }
        "connection.validate" => {
            serde_json::from_value::<fiex_provider_contracts::ConnectionValidation>(value.clone())
                .map_err(|error| format!("invalid connection validation response: {error}"))?;
        }
        "connection.open" | "connection.create" => {
            serde_json::from_value::<fiex_provider_contracts::ConnectionOpenResult>(value.clone())
                .map_err(|error| format!("invalid connection navigation result: {error}"))?;
        }
        _ => {
            if !value.is_object() && !value.is_null() {
                return Err("invalid event result".into());
            }
        }
    }
    Ok(())
}
