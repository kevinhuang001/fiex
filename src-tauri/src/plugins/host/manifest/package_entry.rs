use serde_json::Value;
use std::path::{Component, Path};

/// A declared package entry must stay inside the plugin package. Reads also
/// re-check canonical containment so a symlink cannot escape the package.
pub(crate) fn valid_package_entry(entry: &str) -> bool {
    if entry.is_empty()
        || entry.len() > 1024
        || entry.contains("://")
        || entry.contains(':')
        || entry.contains('\\')
        || entry.contains('\0')
    {
        return false;
    }
    let path = Path::new(entry);
    path.is_relative()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

/// Optional `extensions.preview.loading.entry`, already validated by
/// `validate_extensions` when the manifest is discovered.
pub(crate) fn preview_loading_entry(extensions: &Value) -> Option<&str> {
    extensions
        .get("preview")
        .and_then(Value::as_object)
        .and_then(|preview| preview.get("loading"))
        .and_then(Value::as_object)
        .and_then(|loading| loading.get("entry"))
        .and_then(Value::as_str)
}

#[cfg(test)]
mod tests {
    use super::{preview_loading_entry, valid_package_entry};
    use serde_json::json;

    #[test]
    fn accepts_only_relative_package_paths() {
        for path in ["preview/loading.html", "assets/icon.svg"] {
            assert!(
                valid_package_entry(path),
                "expected {path:?} to be accepted"
            );
        }
        for path in [
            "",
            "/absolute/file.html",
            "../outside.html",
            "assets/../outside.html",
            "https://example.test/preview.html",
            "C:/outside.html",
            "assets\\preview.html",
            "assets\0preview.html",
        ] {
            assert!(
                !valid_package_entry(path),
                "expected {path:?} to be rejected"
            );
        }
        assert!(!valid_package_entry(&"x".repeat(1025)));
    }

    #[test]
    fn reads_only_the_optional_preview_loading_entry() {
        assert_eq!(
            preview_loading_entry(&json!({"preview":{"loading":{"entry":"loading.html"}}})),
            Some("loading.html")
        );
        assert_eq!(preview_loading_entry(&json!({})), None);
        assert_eq!(
            preview_loading_entry(&json!({"preview":{"loading":{"entry":1}}})),
            None
        );
    }
}
