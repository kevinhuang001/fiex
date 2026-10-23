//! Declarative, process-free theme contributions. No CSS selectors or scripts.
use crate::plugin_host::PluginManifest;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Component, Path},
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub mode: String,
    pub tokens: BTreeMap<String, String>,
}
pub fn is_theme(manifest: &PluginManifest) -> bool {
    manifest.runtime.entrypoints.is_empty()
        && manifest
            .contributions
            .get("themes")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|items| !items.is_empty())
}
pub fn load(manifest: &PluginManifest, folder: &Path) -> Result<Vec<Theme>, String> {
    let Some(items) = manifest.contributions.get("themes") else {
        return Ok(Vec::new());
    };
    let items = items.as_array().ok_or("themes must be an array")?;
    if items.len() > 16 {
        return Err("too many themes".into());
    }
    let root = folder.canonicalize().map_err(|e| e.to_string())?;
    let mut themes = Vec::new();
    for item in items {
        let path = item
            .get("path")
            .and_then(serde_json::Value::as_str)
            .ok_or("theme path missing")?;
        if path.is_empty()
            || !Path::new(path)
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
        {
            return Err("unsafe theme path".into());
        }
        let file = root.join(path).canonicalize().map_err(|e| e.to_string())?;
        if !file.starts_with(&root)
            || std::fs::metadata(&file).map_err(|e| e.to_string())?.len() > 32768
        {
            return Err("invalid theme file".into());
        }
        let bytes = std::fs::read(file).map_err(|e| e.to_string())?;
        let mut theme: Theme = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if theme.id.is_empty()
            || theme.id.len() > 64
            || !theme
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c))
            || theme.name.len() > 128
            || !matches!(theme.mode.as_str(), "light" | "dark")
            || theme.tokens.len() > 256
        {
            return Err("invalid theme metadata".into());
        }
        for (key, value) in &theme.tokens {
            if key.is_empty()
                || key.len() > 64
                || !key
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
                || value.len() > 512
                || value.contains([';', '{', '}', '<', '>', '\\'])
                || value.to_ascii_lowercase().contains("url")
                || value.contains('@')
            {
                return Err("invalid theme token".into());
            }
        }
        theme.id = format!("{}:{}", manifest.id, theme.id);
        if themes.iter().any(|t: &Theme| t.id == theme.id) {
            return Err("duplicate theme ID".into());
        }
        themes.push(theme);
    }
    Ok(themes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn themes_are_namespaced_and_cannot_escape_package_or_inject_css() {
        let root = tempfile::tempdir().unwrap();
        let manifest: PluginManifest = serde_json::from_value(json!({"schemaVersion":1,"id":"test.theme","name":"Theme","version":"1","contributions":{"themes":[{"path":"theme.json"}]}})).unwrap();
        let theme =
            json!({"id":"light","name":"Light","mode":"light","tokens":{"accent":"#0067c0"}});
        std::fs::write(root.path().join("theme.json"), theme.to_string()).unwrap();
        assert!(is_theme(&manifest));
        assert_eq!(
            load(&manifest, root.path()).unwrap()[0].id,
            "test.theme:light"
        );
        let mut bad = theme.clone();
        bad["tokens"]["accent"] = json!("red;display:none");
        std::fs::write(root.path().join("theme.json"), bad.to_string()).unwrap();
        assert!(load(&manifest, root.path()).is_err());
        let mut bad_manifest = manifest;
        bad_manifest.contributions["themes"][0]["path"] = json!("../theme.json");
        assert!(load(&bad_manifest, root.path()).is_err());
    }
}
