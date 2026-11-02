use serde_json::Value;
use std::{fs, path::Path};
const MAX_TEXT: usize = 256 * 1024;
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Builtin {
    Text,
    Image,
    Media,
    Pdf,
    Fallback,
}

impl Builtin {
    pub(super) fn id(self) -> &'static str {
        match self {
            Self::Text => "builtin:text",
            Self::Image => "builtin:image",
            Self::Media => "builtin:media",
            Self::Pdf => "builtin:pdf",
            Self::Fallback => "builtin:fallback",
        }
    }

    pub(super) fn priority(self) -> i64 {
        match self {
            Self::Text => 10,
            Self::Image => 20,
            Self::Media => 30,
            Self::Pdf => 40,
            Self::Fallback => 0,
        }
    }
}

pub(super) fn extension(path: &Path) -> String {
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

pub(super) fn builtin_view(kind: Builtin, local: &Path, _display: &str) -> Result<Value, String> {
    let cache_key = (kind == Builtin::Text)
        .then(|| crate::plugins::cache::key("preview.text", local))
        .flatten();
    if let Some(value) = cache_key.as_deref().and_then(crate::plugins::cache::get) {
        return Ok(value);
    }
    let name = name_of(local);
    let ext = extension(local);
    let mime = crate::plugins::match_rules::mime_for(&ext);
    let view = match kind {
        Builtin::Text => {
            use std::io::Read;
            let file = fs::File::open(local).map_err(|e| e.to_string())?;
            let size = file.metadata().map_err(|e| e.to_string())?.len();
            let mut bytes = Vec::new();
            file.take((MAX_TEXT + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
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
    if let Some(key) = cache_key {
        crate::plugins::cache::put(key, view.clone());
    }
    Ok(view)
}
