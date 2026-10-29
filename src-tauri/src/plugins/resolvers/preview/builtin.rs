use serde_json::Value;
use std::{fs, path::Path};
const MAX_TEXT: usize = 256 * 1024;
const TEXT_EXT: &[&str] = &[
    "txt", "md", "markdown", "rst", "log", "csv", "tsv", "json", "json5", "toml", "yaml", "yml",
    "ini", "conf", "cfg", "env", "css", "scss", "less", "html", "htm", "xml", "svg", "sh", "bat",
    "ps1", "py", "rs", "go", "java", "kt", "c", "cc", "cpp", "h", "hpp", "cs", "php", "rb",
    "swift", "sql", "js", "jsx", "ts", "tsx", "vue", "svelte", "dart", "lua", "r", "tex",
];
const IMAGE_EXT: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "tif", "tiff", "avif", "heic",
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Builtin {
    Text,
    Image,
    Media,
    Pdf,
    #[cfg_attr(not(windows), allow(dead_code))]
    OfficePdf,
    Fallback,
}

impl Builtin {
    pub(super) fn id(self) -> &'static str {
        match self {
            Self::Text => "builtin:text",
            Self::Image => "builtin:image",
            Self::Media => "builtin:media",
            Self::Pdf => "builtin:pdf",
            Self::OfficePdf => "builtin:office-pdf",
            Self::Fallback => "builtin:fallback",
        }
    }

    pub(super) fn priority(self) -> i64 {
        match self {
            Self::Text => 10,
            Self::Image => 20,
            Self::Media => 30,
            Self::Pdf => 40,
            Self::OfficePdf => 200,
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

fn is_text(ext: &str, mime: &str) -> bool {
    mime.starts_with("text/")
        || matches!(
            mime,
            "application/json" | "application/xml" | "application/javascript"
        )
        || (mime.is_empty() && TEXT_EXT.contains(&ext))
}

fn is_image(ext: &str, mime: &str) -> bool {
    mime.starts_with("image/") || (mime.is_empty() && IMAGE_EXT.contains(&ext))
}

pub(super) fn builtin_candidates(ext: &str, mime: &str, is_dir: bool) -> Vec<Builtin> {
    if is_dir {
        return vec![Builtin::Fallback];
    }
    let mut values = Vec::new();
    if is_text(ext, mime) {
        values.push(Builtin::Text);
    }
    if is_image(ext, mime) {
        values.push(Builtin::Image);
    }
    if mime.starts_with("video/") || mime.starts_with("audio/") {
        values.push(Builtin::Media);
    }
    if ext == "pdf" {
        values.push(Builtin::Pdf);
    }
    #[cfg(windows)]
    if matches!(ext, "docx" | "pptx") {
        values.push(Builtin::OfficePdf);
    }
    if values.is_empty() {
        values.push(Builtin::Fallback);
    }
    values
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
    let mime = mime_for(&ext);
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
        Builtin::OfficePdf => {
            let path = super::office_pdf::convert(local, &ext)?;
            serde_json::json!({
                "kind": "pdf",
                "path": path.to_string_lossy(),
                "name": name,
                "maxPages": 20,
            })
        }
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

fn mime_for(ext: &str) -> String {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "m4a" => "audio/mp4",
        "json" => "application/json",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "csv" => "text/csv",
        "md" | "markdown" => "text/markdown",
        "txt" | "log" => "text/plain",
        "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "java" | "c" | "cpp" | "h" => {
            "text/plain"
        }
        _ => "application/octet-stream",
    }
    .into()
}
