//! Manifest `match` rules shared by Info, Preview, Decorations and ContextMenu.
//!
//! `match` is an inclusion predicate: Core only invokes the provider when the
//! resource (or directory/selection) satisfies it. Missing or empty rules match
//! everything. Legacy `extensions: [...]` is accepted for third-party manifests.
use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchRules {
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub mime_types: Vec<String>,
    #[serde(default)]
    pub kinds: Vec<String>,
    /// Directory contains one of these names (`.git`, `package.json`, ...).
    /// Decorations only.
    #[serde(default)]
    pub contains: Vec<String>,
    /// Context menu selection cardinality: `single`, `multi` or `any`.
    /// Context menus only.
    #[serde(default)]
    pub selection: Option<String>,
}

impl MatchRules {
    pub fn from_value(value: &Value) -> Self {
        serde_json::from_value(value.clone()).unwrap_or_default()
    }

    /// Read `match` first, then the legacy `extensions` array.
    pub fn from_contribution(contribution: &Value) -> Self {
        if let Some(value) = contribution.get("match") {
            return Self::from_value(value);
        }
        let extensions = contribution
            .get("extensions")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        Self {
            extensions,
            ..Self::default()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.extensions.is_empty()
            && self.mime_types.is_empty()
            && self.kinds.is_empty()
            && self.contains.is_empty()
            && self.selection.is_none()
    }

    pub fn matches(&self, ext: &str, mime: &str, is_dir: bool) -> bool {
        let extension = ext.trim().trim_start_matches('.').to_ascii_lowercase();
        let mime = mime.trim().to_ascii_lowercase();
        let kind = if is_dir { "directory" } else { "file" };
        let extension_ok = self.extensions.is_empty()
            || self.extensions.iter().any(|value| {
                let value = value
                    .trim()
                    .trim_start_matches("*.")
                    .trim_start_matches('.')
                    .to_ascii_lowercase();
                value == "*" || value == extension
            });
        let mime_ok = self.mime_types.is_empty()
            || self.mime_types.iter().any(|pattern| {
                let pattern = pattern.trim().to_ascii_lowercase();
                if pattern == "*/*" || pattern == "*" {
                    return true;
                }
                match pattern.strip_suffix("/*") {
                    Some(prefix) => mime
                        .split('/')
                        .next()
                        .is_some_and(|value| value.eq_ignore_ascii_case(prefix)),
                    None => pattern == mime,
                }
            });
        let kind_ok = self.kinds.is_empty()
            || self.kinds.iter().any(|value| {
                let value = value.trim().to_ascii_lowercase();
                value == kind || (kind == "directory" && value == "dir")
            });
        extension_ok && mime_ok && kind_ok
    }

    /// Directory `contains` names can be in the directory or any ancestor.
    pub fn matches_contains(&self, directory: &str) -> bool {
        if self.contains.is_empty() {
            return true;
        }
        self.contains.iter().any(|marker| {
            let marker = marker.trim();
            if marker.is_empty() || marker.contains(['/', '\\']) {
                return false;
            }
            let mut current = std::path::Path::new(directory);
            loop {
                if current.join(marker).exists() {
                    return true;
                }
                let Some(parent) = current.parent() else {
                    return false;
                };
                current = parent;
            }
        })
    }

    /// Context menu selection cardinality.
    pub fn matches_selection(&self, count: usize) -> bool {
        match self.selection.as_deref().unwrap_or("any") {
            "any" => true,
            "single" => count == 1,
            "multi" => count >= 2,
            _ => false,
        }
    }
}

/// Small extension → MIME map for resources whose directory entry has no MIME.
pub fn mime_for(ext: &str) -> String {
    match ext.trim_start_matches('.').to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "7z" => "application/x-7z-compressed",
        "rar" => "application/vnd.rar",
        "gz" | "tgz" => "application/gzip",
        "tar" => "application/x-tar",
        "bz2" => "application/x-bzip2",
        "xz" => "application/x-xz",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "wmv" => "video/x-ms-wmv",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "m4a" => "audio/mp4",
        "ogg" | "oga" => "audio/ogg",
        "opus" => "audio/opus",
        "aac" => "audio/aac",
        "json" => "application/json",
        "xml" => "application/xml",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "csv" => "text/csv",
        "tsv" => "text/tab-separated-values",
        "md" | "markdown" => "text/markdown",
        "txt" | "log" | "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "java" | "c" | "cpp"
        | "h" | "sh" | "toml" | "yaml" | "yml" | "ini" => "text/plain",
        _ => "application/octet-stream",
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn match_is_an_include_predicate() {
        let rules = MatchRules::from_contribution(&json!({
            "match": {"extensions":["md"],"mimeTypes":["text/*"],"kinds":["file"],"selection":"single"}
        }));
        assert!(rules.matches("md", "text/markdown", false));
        assert!(!rules.matches("png", "image/png", false));
        assert!(!rules.matches("md", "text/markdown", true));
    }

    #[test]
    fn contains_and_selection_are_scoped_predicates() {
        let decorations = MatchRules::from_contribution(&json!({
            "match": {"contains": [".git"], "extensions": ["ts"]}
        }));
        assert!(decorations.matches("ts", "text/plain", false));
        assert!(!decorations.matches("md", "text/markdown", false));
        let menu = MatchRules::from_contribution(&json!({
            "match": {"selection": "single", "extensions": ["zip"]}
        }));
        assert!(menu.matches_selection(1));
        assert!(!menu.matches_selection(2));
        assert!(menu.matches("zip", "application/zip", false));
    }

    #[test]
    fn legacy_extensions_and_wildcards_work() {
        let rules = MatchRules::from_contribution(&json!({"extensions":["*.ZIP","rar"]}));
        assert!(rules.matches("zip", "", false));
        assert!(rules.matches("rar", "", false));
        assert!(!rules.matches("7z", "", false));
        let all = MatchRules::from_contribution(&json!({"match":{"mimeTypes":["text/*"]}}));
        assert!(all.matches("anything", "text/plain", false));
    }
}
