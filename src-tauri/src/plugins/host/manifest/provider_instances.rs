//! Strict manifest validation of multiple named instances within a single
//! provider kind. IDs are plugin-local; Core owns the global namespacing.
use serde_json::{Map, Value};
use std::collections::HashSet;

pub(super) fn validate(feature: &str, contribution: &Map<String, Value>) -> Result<(), String> {
    let Some(instances) = contribution.get("providers") else {
        return Ok(());
    };
    let instances = instances
        .as_array()
        .ok_or_else(|| format!("extensions.{feature}.providers must be an array"))?;
    if instances.is_empty() || instances.len() > 32 {
        return Err(format!(
            "extensions.{feature}.providers expects 1..32 entries"
        ));
    }
    let mut ids = HashSet::new();
    for item in instances {
        let item = item
            .as_object()
            .ok_or_else(|| format!("{feature} provider must be an object"))?;
        if item.keys().any(|key| {
            !matches!(key.as_str(), "id" | "match" | "priority")
                && !(feature == "info" && matches!(key.as_str(), "title" | "subsections"))
        }) {
            return Err(format!(
                "{feature} provider uses an unsupported descriptor field"
            ));
        }
        let id = item
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{feature} provider id required"))?;
        if id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b':'))
            || id.starts_with("core:")
            || id.starts_with("core.")
            || !ids.insert(id)
        {
            return Err(format!("{feature} provider has an invalid or duplicate id"));
        }
        if let Some(priority) = item.get("priority") {
            let value = priority
                .as_i64()
                .ok_or_else(|| format!("{feature} priority must be integer"))?;
            if !(-100_000..=100_000).contains(&value) {
                return Err(format!("{feature} provider priority out of range"));
            }
        }
        crate::plugins::match_rules::validate(item.get("match"), false, feature == "contextMenus")?;
        if feature == "info" {
            if !item.contains_key("match") || !item.contains_key("priority") {
                return Err(
                    "each named Info provider must define its own match and priority".into(),
                );
            }
            if !item.get("title").is_some_and(Value::is_string)
                || !item.get("subsections").is_some_and(Value::is_array)
            {
                return Err(
                    "each named Info provider must define its own title and subsections".into(),
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn rejects_duplicates_forged_core_ids_and_invalid_match() {
        for providers in [
            json!([{"id":"p"},{"id":"p"}]),
            json!([{"id":"core:metadata"}]),
            json!([{"id":"core.metadata"}]),
            json!([{"id":"a","match":{"kinds":["vfs"]}}]),
        ] {
            assert!(validate("info", json!({"providers":providers}).as_object().unwrap()).is_err());
        }
        assert!(
            validate(
                "info",
                json!({"providers":[
                    {"id":"metadata","title":"Metadata","subsections":[{"id":"basic","fields":[{"id":"name","label":"Name","compute":"onOpen"}]}],"priority":100,"match":{}},
                    {"id":"media","title":"Media","subsections":[{"id":"audio","fields":[{"id":"duration","label":"Duration","compute":"onDemand"}]}],"priority":100,"match":{"kinds":["file"]}}
                ]})
                .as_object()
                .unwrap()
            )
            .is_ok()
        );
    }
}
