use serde_json::Value;
use std::collections::HashSet;

pub(super) fn valid_items(value: &Value) -> Result<Vec<Value>, String> {
    let items = value
        .get("items")
        .and_then(Value::as_array)
        .ok_or("menu response requires items[]")?;
    let mut seen = HashSet::new();
    for item in items {
        let id = item
            .get("id")
            .and_then(Value::as_str)
            .ok_or("menu item id missing")?;
        if id.is_empty()
            || id.len() > 80
            || !id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
            || !seen.insert(id.to_owned())
        {
            return Err("invalid or duplicate menu item id".into());
        }
        if !matches!(
            item.get("kind").and_then(Value::as_str),
            Some("action" | "separator")
        ) {
            return Err("invalid connection menu item kind".into());
        }
        if item.get("kind").and_then(Value::as_str) == Some("action") {
            let title = item
                .get("title")
                .and_then(Value::as_str)
                .ok_or("menu item title missing")?;
            if title.is_empty() || title.len() > 128 {
                return Err("invalid menu item title".into());
            }
        }
        if item.get("parentId").is_some() {
            return Err("connection menu items must be flat".into());
        }
    }
    Ok(items.clone())
}
