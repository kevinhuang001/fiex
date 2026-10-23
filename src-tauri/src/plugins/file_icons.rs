//! Plugin-provided file icons.
//!
//! Icons are static assets declared in the manifest. Core validates them at
//! install time and exposes the resolved absolute paths to the browser, which
//! loads them through the Tauri asset protocol. There is deliberately no
//! runtime registration API: a plugin cannot start serving arbitrary files
//! after installation.
use crate::plugin_host::{self, PluginManifest};
use serde::Serialize;
use serde_json::Value;
use std::path::{Component, Path, PathBuf};

const MAX_RESOURCES: usize = 64;
const MAX_RULES: usize = 128;
const MAX_ICON_BYTES: u64 = 256 * 1024;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileIconRule {
    pub plugin_id: String,
    pub plugin_name: String,
    pub resource_id: String,
    pub path: String,
    pub media_type: String,
    pub extensions: Vec<String>,
    pub names: Vec<String>,
    pub mime: Vec<String>,
    pub is_dir: Option<bool>,
    pub priority: i32,
}

#[derive(Clone, Debug)]
struct Resource {
    id: String,
    path: PathBuf,
    media_type: String,
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character))
        && !value.starts_with('.')
}

fn normal_relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn media_type(path: &Path, declared: Option<&str>) -> Option<&'static str> {
    let declared = declared.unwrap_or_default().to_ascii_lowercase();
    if matches!(
        declared.as_str(),
        "image/svg+xml" | "image/png" | "image/jpeg" | "image/webp"
    ) {
        return Some(match declared.as_str() {
            "image/svg+xml" => "image/svg+xml",
            "image/png" => "image/png",
            "image/jpeg" => "image/jpeg",
            _ => "image/webp",
        });
    }
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "svg" => Some("image/svg+xml"),
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

fn svg_is_safe(bytes: &[u8]) -> bool {
    let text = String::from_utf8_lossy(bytes).to_ascii_lowercase();
    ![
        "<script",
        "<foreignobject",
        "<image",
        "javascript:",
        "xlink:href=\"http",
        "xlink:href='http",
        "href=\"http",
        "href='http",
        "url(http",
        "onload=",
        "onerror=",
        "onclick=",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

fn resolve_resource(folder: &Path, path: &str) -> Option<(PathBuf, String)> {
    let relative = Path::new(path);
    if relative.is_absolute() || !normal_relative(relative) {
        return None;
    }
    let full = folder.join(relative);
    let metadata = std::fs::symlink_metadata(&full).ok()?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return None;
    }
    if metadata.len() > MAX_ICON_BYTES {
        return None;
    }
    let canonical_folder = folder.canonicalize().ok()?;
    let canonical_full = full.canonicalize().ok()?;
    if !canonical_full.starts_with(&canonical_folder) {
        return None;
    }
    let media = media_type(&full, None)?;
    if media == "image/svg+xml" {
        let bytes = std::fs::read(&full).ok()?;
        if !svg_is_safe(&bytes) {
            return None;
        }
    }
    Some((full, media.to_string()))
}

fn resources(manifest: &PluginManifest, folder: &Path) -> Vec<Resource> {
    let Some(object) = manifest
        .contributions
        .get("resources")
        .and_then(Value::as_object)
    else {
        return Vec::new();
    };
    object
        .iter()
        .take(MAX_RESOURCES)
        .filter_map(|(id, value)| {
            if !valid_id(id) {
                return None;
            }
            let (path, declared) = match value {
                Value::String(path) => (path.as_str(), None),
                Value::Object(_) => (
                    value.get("path").and_then(Value::as_str)?,
                    value.get("mediaType").and_then(Value::as_str),
                ),
                _ => return None,
            };
            let (path, media) = resolve_resource(folder, path)?;
            if let Some(declared) = declared {
                if media_type(&path, Some(declared)) != Some(media.as_str()) {
                    return None;
                }
            }
            Some(Resource {
                id: id.clone(),
                path,
                media_type: media,
            })
        })
        .collect()
}

pub fn rules(manifest: &PluginManifest, folder: &Path) -> Vec<FileIconRule> {
    let resources = resources(manifest, folder);
    if resources.is_empty() {
        return Vec::new();
    }
    let Some(items) = manifest
        .contributions
        .get("fileIcons")
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for item in items.iter().take(MAX_RULES) {
        let Some(resource_id) = item.get("icon").and_then(Value::as_str) else {
            continue;
        };
        let Some(resource) = resources.iter().find(|entry| entry.id == resource_id) else {
            continue;
        };
        let matcher = item.get("match").unwrap_or(&Value::Null);
        let list = |key: &str| {
            matcher
                .get(key)
                .and_then(Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(Value::as_str)
                        .map(|value| value.trim().trim_start_matches('.').to_ascii_lowercase())
                        .filter(|value| !value.is_empty())
                        .take(64)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };
        let extensions = list("extensions");
        let names = list("names");
        let mime = list("mime");
        let is_dir = matcher.get("isDir").and_then(Value::as_bool);
        if extensions.is_empty() && names.is_empty() && mime.is_empty() && is_dir.is_none() {
            continue;
        }
        result.push(FileIconRule {
            plugin_id: manifest.id.clone(),
            plugin_name: manifest.name.clone(),
            resource_id: resource.id.clone(),
            path: resource.path.to_string_lossy().to_string(),
            media_type: resource.media_type.clone(),
            extensions,
            names,
            mime,
            is_dir,
            priority: item
                .get("priority")
                .and_then(Value::as_i64)
                .unwrap_or(0)
                .clamp(-1000, 1000) as i32,
        });
    }
    result
}

/// Reject a package whose icon declarations point outside the package or at an
/// unsupported asset. Called while the package is still staged.
pub fn validate(manifest: &PluginManifest, folder: &Path) -> Result<(), String> {
    let Some(object) = manifest
        .contributions
        .get("resources")
        .and_then(Value::as_object)
    else {
        return Ok(());
    };
    if object.len() > MAX_RESOURCES {
        return Err("plugin declares too many resources".into());
    }
    for (id, value) in object {
        if !valid_id(id) {
            return Err(format!("invalid resource id: {id}"));
        }
        let (path, declared) = match value {
            Value::String(path) => (path.as_str(), None),
            Value::Object(_) => (
                value
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or_else(|| format!("resource {id} has no path"))?,
                value.get("mediaType").and_then(Value::as_str),
            ),
            _ => return Err(format!("resource {id} must be a path or object")),
        };
        let (resolved, media) = resolve_resource(folder, path)
            .ok_or_else(|| format!("resource {id} is missing or unsafe"))?;
        if let Some(declared) = declared {
            if media_type(&resolved, Some(declared)) != Some(media.as_str()) {
                return Err(format!("resource {id} has an unsupported media type"));
            }
        }
    }
    let Some(items) = manifest
        .contributions
        .get("fileIcons")
        .and_then(Value::as_array)
    else {
        return Ok(());
    };
    if items.len() > MAX_RULES {
        return Err("plugin declares too many file icons".into());
    }
    for item in items {
        let icon = item
            .get("icon")
            .and_then(Value::as_str)
            .ok_or("file icon rule has no icon")?;
        if !object.contains_key(icon) {
            return Err(format!("file icon references unknown resource: {icon}"));
        }
    }
    Ok(())
}

pub fn all() -> Vec<FileIconRule> {
    let mut result = Vec::new();
    for (manifest, folder) in plugin_host::installed_manifests() {
        let declared = plugin_host::declared_permissions(&manifest).unwrap_or_default();
        let Ok(status) = crate::plugin_permissions::status(&manifest.id, &declared) else {
            continue;
        };
        if !status.enabled {
            continue;
        }
        result.extend(rules(&manifest, &folder));
    }
    result.sort_by(|a, b| {
        b.priority
            .cmp(&a.priority)
            .then_with(|| a.plugin_id.cmp(&b.plugin_id))
            .then_with(|| a.resource_id.cmp(&b.resource_id))
    });
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsafe_svg() {
        assert!(svg_is_safe(
            br#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#
        ));
        assert!(!svg_is_safe(br#"<svg><script>alert(1)</script></svg>"#));
        assert!(!svg_is_safe(
            br#"<svg><image href="https://example.com/a.png"/></svg>"#
        ));
    }

    #[test]
    fn resolves_declared_resources() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("assets")).unwrap();
        std::fs::write(
            dir.path().join("assets/rust.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#,
        )
        .unwrap();
        let manifest: PluginManifest = serde_json::from_value(serde_json::json!({
            "schemaVersion": 1,
            "id": "org.test.icons",
            "name": "Test Icons",
            "version": "1.0.0",
            "contributions": {
                "resources": {
                    "rust": { "path": "assets/rust.svg", "mediaType": "image/svg+xml" }
                },
                "fileIcons": [
                    { "match": { "extensions": [".RS"] }, "icon": "rust", "priority": 10 }
                ]
            }
        }))
        .unwrap();
        assert!(validate(&manifest, dir.path()).is_ok());
        let rules = rules(&manifest, dir.path());
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].extensions, vec!["rs"]);
        assert!(rules[0].path.ends_with("assets/rust.svg"));
    }

    #[test]
    fn media_types_are_limited() {
        assert_eq!(media_type(Path::new("a.svg"), None), Some("image/svg+xml"));
        assert_eq!(media_type(Path::new("a.png"), None), Some("image/png"));
        assert_eq!(media_type(Path::new("a.html"), None), None);
    }
}
