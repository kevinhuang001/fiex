pub(super) fn menu_response_valid(
    result: &serde_json::Value,
) -> Result<std::collections::HashSet<String>, String> {
    use std::collections::{HashMap, HashSet};
    let items = result
        .get("items")
        .and_then(serde_json::Value::as_array)
        .ok_or("menu response requires items[]")?;
    if items.len() > 100 {
        return Err("too many menu items".into());
    }
    let mut ids = HashSet::new();
    let mut kinds = HashMap::new();
    let mut parents = HashMap::new();
    let mut actions = HashSet::new();
    for item in items {
        let id = item
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or("menu item id missing")?;
        if id.is_empty()
            || id.len() > 80
            || !id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
            || !ids.insert(id.to_owned())
        {
            return Err("invalid or duplicate menu item id".into());
        }
        let kind = item
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .ok_or("menu kind missing")?;
        if !matches!(kind, "action" | "submenu" | "separator") {
            return Err("invalid menu item kind".into());
        }
        let title = item
            .get("title")
            .and_then(serde_json::Value::as_str)
            .ok_or("menu title missing")?;
        if title.is_empty() || title.len() > 128 {
            return Err("invalid menu item title".into());
        }
        if item.get("when").is_some() || item.get("children").is_some() {
            return Err("static nested menus are forbidden".into());
        }
        if let Some(parent) = item.get("parentId") {
            parents.insert(
                id.to_owned(),
                parent.as_str().ok_or("invalid menu parent")?.to_owned(),
            );
        }
        if item
            .get("disabled")
            .is_some_and(|value| !value.is_boolean())
        {
            return Err("invalid menu disabled flag".into());
        }
        if kind == "action" && item["disabled"] != true {
            actions.insert(id.to_owned());
        }
        kinds.insert(id.to_owned(), kind.to_owned());
    }
    for id in &ids {
        let mut current = id.as_str();
        let mut depth = 1;
        let mut seen = HashSet::new();
        while let Some(parent) = parents.get(current) {
            if !seen.insert(current) || kinds.get(parent).map(String::as_str) != Some("submenu") {
                return Err("menu contains a cycle or invalid parent".into());
            }
            depth += 1;
            if depth > 4 {
                return Err("menu nesting exceeds depth limit".into());
            }
            current = parent;
        }
    }
    Ok(actions)
}
/// Track submenu depth so a lazy expansion cannot exceed the four-level cap.
pub(super) fn root_menu_valid(
    result: &serde_json::Value,
) -> Result<std::collections::HashSet<String>, String> {
    let items = result
        .get("items")
        .and_then(serde_json::Value::as_array)
        .ok_or("menu response requires items[]")?;
    if items.iter().any(|item| item.get("parentId").is_some()) {
        return Err("root menu must not include submenu children".into());
    }
    menu_response_valid(result)
}

pub(super) fn submenu_depths(
    result: &serde_json::Value,
) -> std::collections::HashMap<String, usize> {
    use std::collections::HashMap;
    let Some(items) = result.get("items").and_then(serde_json::Value::as_array) else {
        return HashMap::new();
    };
    let by_id: HashMap<&str, &serde_json::Value> = items
        .iter()
        .filter_map(|item| {
            item.get("id")
                .and_then(serde_json::Value::as_str)
                .map(|id| (id, item))
        })
        .collect();
    let mut depths = HashMap::new();
    for item in items {
        if item.get("kind").and_then(serde_json::Value::as_str) != Some("submenu") {
            continue;
        }
        let Some(id) = item.get("id").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let mut depth = 1;
        let mut current = item;
        while let Some(parent) = current.get("parentId").and_then(serde_json::Value::as_str) {
            depth += 1;
            let Some(ancestor) = by_id.get(parent) else {
                break;
            };
            current = ancestor;
        }
        depths.insert(id.to_string(), depth);
    }
    depths
}

/// Validate each lazy submenu response independently. The plugin cannot
/// supply the parent id or forge actions from another menu session.
pub(super) struct ValidatedSubmenu {
    pub items: Vec<serde_json::Value>,
    pub actions: std::collections::HashSet<String>,
    pub submenus: std::collections::HashMap<String, usize>,
}

pub(super) fn validated_submenu(
    value: &serde_json::Value,
    parent_id: &str,
    parent_depth: usize,
) -> Result<ValidatedSubmenu, String> {
    let children = value
        .get("items")
        .and_then(serde_json::Value::as_array)
        .ok_or("submenu response requires items[]")?;
    if children.len() > 99 || (parent_depth >= 4 && !children.is_empty()) {
        return Err("submenu exceeds item/depth limit".into());
    }
    let mut items = Vec::with_capacity(children.len() + 1);
    items.push(serde_json::json!({"id":parent_id,"kind":"submenu","title":"parent"}));
    for child in children {
        let mut child = child
            .as_object()
            .ok_or("submenu entry must be an object")?
            .clone();
        if child.contains_key("parentId") {
            return Err("submenu children must not supply parentId".into());
        }
        child.insert("parentId".into(), serde_json::json!(parent_id));
        items.push(serde_json::Value::Object(child));
    }
    let response = serde_json::json!({"items":items});
    let actions = menu_response_valid(&response)?;
    let submenus = submenu_depths(&response)
        .into_iter()
        .filter(|(id, _)| id != parent_id)
        .map(|(id, _)| (id, parent_depth + 1))
        .collect();
    Ok(ValidatedSubmenu {
        items: items.into_iter().skip(1).collect(),
        actions,
        submenus,
    })
}
