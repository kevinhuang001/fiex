//! Local built-in preview rendering primitives.
//!
//! Provider matching, resource authorization, and cache lifetime remain in
//! the application runtime. MIME lookup is injected so this crate has no
//! dependency on Core's provider registry.

use serde_json::Value;
use std::{fs, path::Path};

const MAX_TEXT: usize = 256 * 1024;

pub const MODULE_ID: &str = "core:preview";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Builtin {
    Text,
    Image,
    Media,
    Pdf,
    Fallback,
}

mod providers;

pub use providers::{PreviewModule, PreviewModuleServices, PreviewProviderRegistration};

impl Builtin {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Text => "builtin:text",
            Self::Image => "builtin:image",
            Self::Media => "builtin:media",
            Self::Pdf => "builtin:pdf",
            Self::Fallback => "builtin:fallback",
        }
    }

    pub const fn priority(self) -> i64 {
        match self {
            Self::Text => 10,
            Self::Image => 20,
            Self::Media => 30,
            Self::Pdf => 40,
            Self::Fallback => 0,
        }
    }
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// Render a built-in preview model. Text reads are capped at 256 KiB.
pub fn render(
    kind: Builtin,
    local: &Path,
    mime_for: impl FnOnce(&str) -> String,
) -> Result<Value, String> {
    let name = name_of(local);
    let ext = extension(local);
    let mime = mime_for(&ext);
    let view = match kind {
        Builtin::Text => {
            use std::io::Read;
            let file = fs::File::open(local).map_err(|error| error.to_string())?;
            let size = file.metadata().map_err(|error| error.to_string())?.len();
            let mut bytes = Vec::new();
            file.take((MAX_TEXT + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|error| error.to_string())?;
            let truncated = bytes.len() > MAX_TEXT;
            let text = String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_TEXT)]).into_owned();
            serde_json::json!({
                "kind": "text",
                "text": text,
                "truncated": truncated,
                "size": size,
            })
        }
        Builtin::Image => serde_json::json!({
            "kind": "image",
            "path": local.to_string_lossy(),
            "name": name,
            "mime": mime,
        }),
        Builtin::Media => serde_json::json!({
            "kind": "media",
            "media": if mime.starts_with("video/") { "video" } else { "audio" },
            "path": local.to_string_lossy(),
            "name": name,
            "mime": mime,
        }),
        Builtin::Pdf => serde_json::json!({
            "kind": "pdf",
            "path": local.to_string_lossy(),
            "name": name,
        }),
        Builtin::Fallback => serde_json::json!({
            "kind": "fallback",
            "name": name,
            "extension": ext,
        }),
    };
    Ok(view)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_preview_is_bounded_and_reports_original_size() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("large.txt");
        fs::write(&path, vec![b'x'; MAX_TEXT + 20]).unwrap();

        let view = render(Builtin::Text, &path, |_| "text/plain".into()).unwrap();
        assert_eq!(view["text"].as_str().unwrap().len(), MAX_TEXT);
        assert_eq!(view["truncated"], true);
        assert_eq!(view["size"], (MAX_TEXT + 20) as u64);
    }

    #[test]
    fn media_preview_classifies_video_and_keeps_original_path() {
        let path = Path::new("/tmp/clip.mp4");
        let view = render(Builtin::Media, path, |_| "video/mp4".into()).unwrap();
        assert_eq!(view["kind"], "media");
        assert_eq!(view["media"], "video");
        assert_eq!(view["path"], "/tmp/clip.mp4");
        assert_eq!(view["name"], "clip.mp4");
    }

    #[test]
    fn fallback_does_not_read_the_file() {
        let path = Path::new("missing.unknown");
        let view = render(Builtin::Fallback, path, |_| {
            "application/octet-stream".into()
        })
        .unwrap();
        assert_eq!(view["kind"], "fallback");
        assert_eq!(view["extension"], "unknown");
    }
}
