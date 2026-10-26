//! Namespaced static resources declared by plugins.
//!
//! A plugin can ship icons/images in its package and reference them from
//! dynamic decorations as `fiex-resource://<plugin-id>/<resource-id>`. Core
//! validates the path, media type and SVG content at install time, and resolves
//! the reference only when the namespace matches the decorating plugin.
use crate::plugin_host::PluginManifest;
use serde_json::Value;
use std::path::{Component, Path, PathBuf};

const MAX_RESOURCES: usize = 64;
const MAX_RESOURCE_BYTES: u64 = 256 * 1024;

#[derive(Clone, Debug)]
struct Resource {
    id: String,
    path: PathBuf,
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

fn resolve_file(folder: &Path, path: &str) -> Option<(PathBuf, String)> {
    let relative = Path::new(path);
    if relative.is_absolute() || !normal_relative(relative) {
        return None;
    }
    let full = folder.join(relative);
    let metadata = std::fs::symlink_metadata(&full).ok()?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return None;
    }
    if metadata.len() > MAX_RESOURCE_BYTES {
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
        .extensions
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
            let (path, media) = resolve_file(folder, path)?;
            if let Some(declared) = declared {
                if media_type(&path, Some(declared)) != Some(media.as_str()) {
                    return None;
                }
            }
            Some(Resource {
                id: id.clone(),
                path,
            })
        })
        .collect()
}

/// Parse `fiex-resource://<plugin-id>/<resource-id>` and enforce that the
/// namespace is exactly the plugin producing the decoration.
pub fn resolve(
    manifest: &PluginManifest,
    folder: &Path,
    reference: &str,
) -> Option<(String, String)> {
    let rest = reference.strip_prefix("fiex-resource://")?;
    let (namespace, resource_id) = rest.split_once('/')?;
    if namespace != manifest.id || !valid_id(resource_id) {
        return None;
    }
    let resource = resources(manifest, folder)
        .into_iter()
        .find(|entry| entry.id == resource_id)?;
    Some((
        format!("fiex-resource://{namespace}/{resource_id}"),
        resource.path.to_string_lossy().to_string(),
    ))
}

/// Reject a package whose resources point outside the package or use an
/// unsupported type. Called while the package is still staged.
pub fn validate(manifest: &PluginManifest, folder: &Path) -> Result<(), String> {
    let Some(object) = manifest
        .extensions
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
        let (resolved, media) = resolve_file(folder, path)
            .ok_or_else(|| format!("resource {id} is missing or unsafe"))?;
        if let Some(declared) = declared {
            if media_type(&resolved, Some(declared)) != Some(media.as_str()) {
                return Err(format!("resource {id} has an unsupported media type"));
            }
        }
    }
    Ok(())
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
    fn media_types_are_limited() {
        assert_eq!(media_type(Path::new("a.svg"), None), Some("image/svg+xml"));
        assert_eq!(media_type(Path::new("a.png"), None), Some("image/png"));
        assert_eq!(media_type(Path::new("a.html"), None), None);
    }

    #[test]
    fn resolves_only_its_own_namespace() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("assets")).unwrap();
        std::fs::write(
            dir.path().join("assets/rust.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#,
        )
        .unwrap();
        let manifest: PluginManifest = serde_json::from_value(serde_json::json!({
            "schemaVersion": 2,
            "id": "org.test.icons",
            "name": "Test Icons",
            "version": "1.0.0",
            "extensions": {
                "resources": {
                    "rust": { "path": "assets/rust.svg", "mediaType": "image/svg+xml" }
                }
            }
        }))
        .unwrap();
        assert!(validate(&manifest, dir.path()).is_ok());
        let (namespace, path) =
            resolve(&manifest, dir.path(), "fiex-resource://org.test.icons/rust").unwrap();
        assert_eq!(namespace, "fiex-resource://org.test.icons/rust");
        assert!(path.ends_with("assets/rust.svg"));
        assert!(resolve(
            &manifest,
            dir.path(),
            "fiex-resource://org.other.icons/rust"
        )
        .is_none());
    }
}
