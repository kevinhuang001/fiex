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
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dark_tokens: BTreeMap<String, String>,
}
fn validate_tokens(tokens: &BTreeMap<String, String>) -> Result<(), String> {
    for (key, value) in tokens {
        // Must be a valid custom-property suffix: a lowercase letter first,
        // then lowercase letters, digits or dashes. The UI SDK and the host
        // filter tokens with the same rule, so anything else would install
        // and then be silently ignored.
        let mut chars = key.chars();
        let valid_start = chars.next().is_some_and(|c| c.is_ascii_lowercase());
        if key.is_empty()
            || key.len() > 64
            || !valid_start
            || !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            || value.len() > 512
            || value.contains([';', '{', '}', '<', '>', '\\'])
            || value.to_ascii_lowercase().contains("url")
            || value.contains('@')
        {
            return Err("invalid theme token".into());
        }
    }
    Ok(())
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
            // A blank name would render as an empty row in the appearance picker.
            || theme.name.trim().is_empty()
            || theme.name.len() > 128
            || !matches!(theme.mode.as_str(), "light" | "dark")
            || theme.tokens.len() + theme.dark_tokens.len() > 256
        {
            return Err("invalid theme metadata".into());
        }
        validate_tokens(&theme.tokens)?;
        validate_tokens(&theme.dark_tokens)?;
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
    use serde_json::{json, Value};

    fn package(themes: Value, entrypoints: Value) -> PluginManifest {
        serde_json::from_value(json!({
            "schemaVersion": 1,
            "id": "test.theme",
            "name": "Theme",
            "version": "1.0.0",
            "runtime": { "protocolVersion": "2.0", "entrypoints": entrypoints },
            "contributions": { "themes": themes }
        }))
        .unwrap()
    }

    fn theme(id: &str) -> Value {
        json!({
            "id": id,
            "name": "Light",
            "mode": "light",
            "tokens": { "accent": "#0067c0" }
        })
    }

    fn write(dir: &Path, name: &str, value: &Value) {
        std::fs::write(dir.join(name), serde_json::to_vec(value).unwrap()).unwrap();
    }

    #[test]
    fn themes_are_namespaced_and_cannot_escape_package_or_inject_css() {
        let root = tempfile::tempdir().unwrap();
        let manifest = package(json!([{ "path": "theme.json" }]), json!({}));
        write(root.path(), "theme.json", &theme("light"));
        assert!(is_theme(&manifest));
        assert_eq!(
            load(&manifest, root.path()).unwrap()[0].id,
            "test.theme:light"
        );
        let mut bad = theme("light");
        bad["tokens"]["accent"] = json!("red;display:none");
        write(root.path(), "theme.json", &bad);
        assert!(load(&manifest, root.path()).is_err());
        let mut bad_manifest = manifest;
        bad_manifest.contributions["themes"][0]["path"] = json!("../theme.json");
        assert!(load(&bad_manifest, root.path()).is_err());
    }

    #[test]
    fn themes_can_carry_a_dark_variant() {
        let root = tempfile::tempdir().unwrap();
        let manifest = package(json!([{ "path": "theme.json" }]), json!({}));
        let mut value = theme("adaptive");
        value["darkTokens"] = json!({ "accent": "#4cc2ff", "surface": "#202020" });
        write(root.path(), "theme.json", &value);
        let loaded = load(&manifest, root.path()).unwrap();
        assert_eq!(loaded[0].dark_tokens["accent"], "#4cc2ff");

        value["darkTokens"]["accent"] = json!("url(https://example.invalid/a.png)");
        write(root.path(), "theme.json", &value);
        assert!(load(&manifest, root.path()).is_err());
    }

    #[test]
    fn only_pure_contributions_count_as_themes() {
        let pure = package(json!([{ "path": "theme.json" }]), json!({}));
        assert!(is_theme(&pure));
        let native = package(
            json!([{ "path": "theme.json" }]),
            json!({ "linux-x86_64": "bin" }),
        );
        assert!(!is_theme(&native));
        let empty = package(json!([]), json!({}));
        assert!(!is_theme(&empty));
    }

    #[test]
    fn rejects_css_injection_and_urls_in_token_values() {
        let root = tempfile::tempdir().unwrap();
        let manifest = package(json!([{ "path": "theme.json" }]), json!({}));
        for value in [
            "red;display:none",
            "url(https://example.invalid/a.png)",
            "URL(https://example.invalid/a.png)",
            "@import 'x.css'",
            "a{b}",
            "<script>alert(1)</script>",
            "back\\slash",
        ] {
            let mut bad = theme("light");
            bad["tokens"]["accent"] = json!(value);
            write(root.path(), "theme.json", &bad);
            assert!(
                load(&manifest, root.path()).is_err(),
                "token value must be rejected: {value}"
            );
        }
    }

    #[test]
    fn rejects_token_names_that_cannot_become_custom_properties() {
        let root = tempfile::tempdir().unwrap();
        let manifest = package(json!([{ "path": "theme.json" }]), json!({}));
        for key in ["Accent", "accent_soft", "", "1accent", "accent\u{1F600}"] {
            let mut bad = theme("light");
            bad["tokens"] = json!({ key: "#000000" });
            write(root.path(), "theme.json", &bad);
            assert!(
                load(&manifest, root.path()).is_err(),
                "token name must be rejected: {key:?}"
            );
        }
        let long = "a".repeat(65);
        let mut bad = theme("light");
        bad["tokens"] = json!({ long.clone(): "#000000" });
        write(root.path(), "theme.json", &bad);
        assert!(load(&manifest, root.path()).is_err());
    }

    #[test]
    fn accepts_gradients_shadows_and_custom_token_names() {
        let root = tempfile::tempdir().unwrap();
        let manifest = package(json!([{ "path": "theme.json" }]), json!({}));
        let mut theme = theme("aero");
        theme["tokens"] = json!({
            "accent": "#1e6fb8",
            "accent-soft": "color-mix(in srgb, #1e6fb8 12%, transparent)",
            "toolbar-background": "linear-gradient(180deg, #f4fbff 0%, #c3e0f4 54%, #d8ecfa 100%)",
            "row-selected-background": "linear-gradient(180deg, #f3f9fc 45%, #d9eaf5)",
            "shadow": "0 1px 0 #ffffffcc inset, 0 12px 34px #1b4b6e33",
            "radius-control": "3px",
            "font-family": "Segoe UI, Noto Sans SC, sans-serif"
        });
        write(root.path(), "theme.json", &theme);
        let loaded = load(&manifest, root.path()).unwrap();
        assert_eq!(loaded[0].id, "test.theme:aero");
        assert_eq!(loaded[0].tokens["accent"], "#1e6fb8");
        assert!(loaded[0].tokens["toolbar-background"].contains("gradient"));
        assert!(loaded[0].tokens.contains_key("accent-soft"));
    }

    #[test]
    fn rejects_invalid_metadata_and_duplicate_ids() {
        let root = tempfile::tempdir().unwrap();
        let manifest = package(json!([{ "path": "theme.json" }]), json!({}));
        for (field, value) in [
            ("mode", json!("auto")),
            ("name", json!("")),
            ("id", json!("has space")),
            ("id", json!("a".repeat(65))),
        ] {
            let mut bad = theme("light");
            bad[field] = value;
            write(root.path(), "theme.json", &bad);
            assert!(load(&manifest, root.path()).is_err(), "field {field}");
        }
        let mut unknown = theme("light");
        unknown["extends"] = json!("dark");
        write(root.path(), "theme.json", &unknown);
        assert!(load(&manifest, root.path()).is_err());

        write(root.path(), "a.json", &theme("same"));
        write(root.path(), "b.json", &theme("same"));
        let duplicate = package(
            json!([{ "path": "a.json" }, { "path": "b.json" }]),
            json!({}),
        );
        assert!(load(&duplicate, root.path()).is_err());
    }

    #[test]
    fn rejects_too_many_theme_entries_and_oversized_files() {
        let root = tempfile::tempdir().unwrap();
        let entries: Vec<Value> = (0..17).map(|_| json!({ "path": "theme.json" })).collect();
        let many = package(json!(entries), json!({}));
        assert!(load(&many, root.path()).is_err());

        let manifest = package(json!([{ "path": "theme.json" }]), json!({}));
        let mut huge = theme("light");
        huge["tokens"]["accent"] = json!("a".repeat(33 * 1024));
        write(root.path(), "theme.json", &huge);
        assert!(load(&manifest, root.path()).is_err());
    }

    #[test]
    fn rejects_absolute_and_nested_escape_paths() {
        let root = tempfile::tempdir().unwrap();
        let manifest = package(json!([{ "path": "theme.json" }]), json!({}));
        write(root.path(), "theme.json", &theme("light"));
        for path in ["/etc/passwd", "ui/../theme.json", "a/b.json", ""] {
            let mut bad = manifest.clone();
            bad.contributions["themes"][0]["path"] = json!(path);
            assert!(
                load(&bad, root.path()).is_err(),
                "path must be rejected: {path:?}"
            );
        }
    }

    #[test]
    fn requires_every_theme_to_parse() {
        let root = tempfile::tempdir().unwrap();
        let manifest = package(
            json!([{ "path": "good.json" }, { "path": "bad.json" }]),
            json!({}),
        );
        write(root.path(), "good.json", &theme("good"));
        std::fs::write(root.path().join("bad.json"), b"{ not json").unwrap();
        assert!(load(&manifest, root.path()).is_err());
    }
}
