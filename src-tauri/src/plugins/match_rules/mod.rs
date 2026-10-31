//! Manifest `match` rules shared by Info, Preview, Decorations and ContextMenu.
//!
//! `match` is an inclusion predicate: Core only invokes the provider when the
//! resource (or directory/selection) satisfies it. Missing or empty rules match
//! everything.
use serde::Deserialize;
use serde_json::Value;

mod location;
pub use location::locations_for;

/// Eligibility of a *trusted* view or storage owner, not a file kind.
/// Each field is OR internally, all specified dimensions are AND.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocationMatch {
    #[serde(default)]
    pub types: Option<Vec<String>>,
    #[serde(default)]
    pub provider_ids: Option<Vec<String>>,
    #[serde(default)]
    pub mount_ids: Option<Vec<String>>,
}

impl LocationMatch {
    fn matches(&self, location: &Value) -> bool {
        let field_matches = |field: &str, accepted: &Option<Vec<String>>| {
            accepted.as_ref().is_none_or(|values| {
                location
                    .get(field)
                    .and_then(Value::as_str)
                    .is_some_and(|actual| values.iter().any(|value| value == actual))
            })
        };
        location.is_object()
            && field_matches("type", &self.types)
            && field_matches("providerId", &self.provider_ids)
            && field_matches("mountId", &self.mount_ids)
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MatchRules {
    #[serde(default)]
    pub view: Option<LocationMatch>,
    #[serde(default)]
    pub storage: Option<LocationMatch>,
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub mime_types: Vec<String>,
    #[serde(default)]
    pub kinds: Vec<String>,
    #[serde(default)]
    pub required_ops: Vec<crate::plugin_sdk::filesystem::FsOperation>,
    /// Context menu selection cardinality: `zero`, `single`, `multi` or `any`.
    /// Context menus only.
    #[serde(default)]
    pub selection: Option<String>,
}

impl MatchRules {
    /// Invalid predicates never degrade into an unrestricted provider.
    pub fn from_contribution(contribution: &Value) -> Result<Self, serde_json::Error> {
        contribution
            .get("match")
            .map(|value| serde_json::from_value(value.clone()))
            .unwrap_or_else(|| Ok(Self::default()))
    }

    pub fn matches_event(&self, metadata: &[Value], directory: &str) -> bool {
        if !self.matches_selection(metadata.len()) {
            return false;
        }
        // For blank-area menus, evaluate the real directory rather than
        // treating an empty selection as permission to bypass location/caps.
        let blank = (metadata.is_empty() && !directory.is_empty())
            .then(|| location::directory_metadata(directory))
            .flatten();
        let targets: &[Value] = if metadata.is_empty() {
            blank.as_ref().map(std::slice::from_ref).unwrap_or(&[])
        } else {
            metadata
        };
        let has_target_predicate = !self.extensions.is_empty()
            || !self.mime_types.is_empty()
            || !self.kinds.is_empty()
            || !self.required_ops.is_empty()
            || self.view.is_some()
            || self.storage.is_some();
        if targets.is_empty() {
            return !has_target_predicate;
        }
        targets.iter().all(|m| {
            let kind = match m["kind"].as_str() {
                Some("directory") => Some(true),
                Some("file") => Some(false),
                _ => m["isDir"].as_bool(),
            };
            m.is_object()
                && (self.extensions.is_empty() || m["extension"].is_string())
                && (self.mime_types.is_empty() || m["mime"].is_string())
                && (self.kinds.is_empty() || kind.is_some())
                && self.matches(
                    m["extension"].as_str().unwrap_or(""),
                    m["mime"].as_str().unwrap_or(""),
                    kind.unwrap_or(false),
                )
                && self
                    .view
                    .as_ref()
                    .is_none_or(|rule| rule.matches(&m["view"]))
                && self
                    .storage
                    .as_ref()
                    .is_none_or(|rule| rule.matches(&m["storage"]))
                && self.required_ops.iter().all(|op| {
                    m["operations"].as_array().is_some_and(|ops| {
                        ops.iter().any(|value| value.as_str() == Some(op.as_str()))
                    })
                })
        })
    }

    pub fn is_empty(&self) -> bool {
        self.view.is_none()
            && self.storage.is_none()
            && self.extensions.is_empty()
            && self.mime_types.is_empty()
            && self.kinds.is_empty()
            && self.selection.is_none()
            && self.required_ops.is_empty()
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

    /// Context menu selection cardinality. `zero` is the blank-area menu; a
    /// manifest that wants both blank and selected menus uses `any`.
    pub fn matches_selection(&self, count: usize) -> bool {
        match self.selection.as_deref().unwrap_or("any") {
            "any" => true,
            "zero" => count == 0,
            "single" => count == 1,
            "multi" => count >= 2,
            _ => false,
        }
    }
}

pub fn validate(
    value: Option<&Value>,
    _allow_contains: bool,
    allow_selection: bool,
) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    let object = value.as_object().ok_or("match must be an object")?;
    if object.contains_key("contains") {
        return Err("Feature matching cannot depend on a host directory".into());
    }
    if !allow_selection && object.contains_key("selection") {
        return Err("match.selection is only valid for contextMenus".into());
    }
    for key in ["extensions", "mimeTypes", "kinds"] {
        let Some(items) = object.get(key) else {
            continue;
        };
        let items = items
            .as_array()
            .ok_or_else(|| format!("match.{key} must be an array"))?;
        if items.iter().any(|item| !item.is_string()) {
            return Err(format!("match.{key} entries must be strings"));
        }
    }
    for (label, location) in [
        ("view", object.get("view")),
        ("storage", object.get("storage")),
    ] {
        let Some(location) = location else { continue };
        let fields = location
            .as_object()
            .ok_or_else(|| format!("match.{label} must be an object"))?;
        if fields.is_empty() {
            return Err(format!(
                "match.{label} must constrain types, providerIds or mountIds"
            ));
        }
        for (key, values) in fields {
            if !matches!(key.as_str(), "types" | "providerIds" | "mountIds") {
                return Err(format!("match.{label}.{key} is unknown"));
            }
            let values = values
                .as_array()
                .ok_or_else(|| format!("match.{label}.{key} must be an array"))?;
            if values.is_empty() || values.len() > 64 {
                return Err(format!("match.{label}.{key} must have 1..64 entries"));
            }
            for value in values {
                let item = value
                    .as_str()
                    .ok_or_else(|| format!("match.{label}.{key} requires strings"))?;
                if item.is_empty() || item.len() > 256 || item.chars().any(char::is_control) {
                    return Err(format!("match.{label}.{key} contains an invalid value"));
                }
                if key == "types" && !matches!(item, "fs" | "vfs") {
                    return Err(format!("match.{label}.types accepts only fs or vfs"));
                }
            }
        }
    }
    if let Some(kinds) = object.get("kinds").and_then(Value::as_array) {
        if kinds
            .iter()
            .any(|item| !matches!(item.as_str(), Some("file" | "directory" | "connection")))
        {
            return Err("match.kinds accepts only file, directory or connection".into());
        }
    }
    if let Some(selection) = object.get("selection") {
        let selection = selection
            .as_str()
            .ok_or("match.selection must be any, zero, single or multi")?;
        if !matches!(selection, "any" | "zero" | "single" | "multi") {
            return Err("match.selection must be any, zero, single or multi".into());
        }
    }
    serde_json::from_value::<MatchRules>(value.clone()).map_err(|error| error.to_string())?;
    Ok(())
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
mod tests;
