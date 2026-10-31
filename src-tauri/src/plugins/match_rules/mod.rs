//! Manifest `match` rules shared by Info, Preview, Decorations and ContextMenu.
//!
//! `match` is an inclusion predicate: Core only invokes the provider when the
//! resource (or directory/selection) satisfies it. Missing or empty rules match
//! everything.
use serde_json::Value;
use std::ops::Deref;

mod location;
pub use location::locations_for;

/// Core uses the SDK's one authoritative manifest predicate and DTO.
/// This thin adapter only supplies a trusted directory snapshot for blank
/// canvas events; it never defines independent match/selection semantics.
#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(transparent)]
pub struct MatchRules(pub fiex_provider_contracts::ProviderMatchRule);

impl Deref for MatchRules {
    type Target = fiex_provider_contracts::ProviderMatchRule;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl MatchRules {
    pub fn from_contribution(contribution: &Value) -> Result<Self, serde_json::Error> {
        contribution
            .get("match")
            .map(|value| serde_json::from_value(value.clone()))
            .unwrap_or_else(|| Ok(Self::default()))
    }

    pub fn matches_event(&self, metadata: &[Value], directory: &str) -> bool {
        let blank = (metadata.is_empty() && !directory.is_empty())
            .then(|| location::directory_metadata(directory))
            .flatten();
        let targets = if metadata.is_empty() {
            blank.as_slice()
        } else {
            metadata
        };
        self.0.matches_targets(targets, metadata.len())
    }
}

pub fn validate(
    value: Option<&Value>,
    _allow_contains: bool,
    allow_selection: bool,
) -> Result<(), String> {
    fiex_provider_contracts::ProviderMatchRule::validate(value, allow_selection)
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
