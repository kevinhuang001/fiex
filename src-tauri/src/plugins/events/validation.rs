use serde_json::Value;
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
        "info.resolve" => {
            let sections = array("sections", 64)?;
            let mut ids = std::collections::HashSet::new();
            for s in sections {
                for key in ["id", "title"] {
                    if s[key]
                        .as_str()
                        .filter(|s| !s.is_empty() && s.len() <= 1024)
                        .is_none()
                    {
                        return Err("invalid info section".into());
                    }
                }
                if !ids.insert(s["id"].as_str().unwrap()) {
                    return Err("duplicate info section id".into());
                }
                let rows = s["rows"]
                    .as_array()
                    .filter(|a| a.len() <= 256)
                    .ok_or("invalid info rows")?;
                for r in rows {
                    if r["label"].as_str().filter(|s| s.len() <= 16384).is_none()
                        || r["value"].as_str().filter(|s| s.len() <= 16384).is_none()
                        || r.get("copyable").is_some_and(|v| !v.is_boolean())
                    {
                        return Err("invalid info row".into());
                    }
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
        "decorations.resolve" => {
            array("decorations", 100_000)?;
        }
        "search.query" => {
            array("entries", 500)?;
        }
        "hooks.invoke" => {
            if value["handled"].as_bool().is_none()
                || value.get("preview").is_some_and(|v| !v.is_boolean())
            {
                return Err("invalid hook response".into());
            }
        }
        _ => {
            if !value.is_object() && !value.is_null() {
                return Err("invalid event result".into());
            }
        }
    }
    Ok(())
}
