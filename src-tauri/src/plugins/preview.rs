//! Unified preview registry: built-ins and plugins return the same resolved view.
//!
//! Providers are matched by extension/MIME and priority. Built-ins use a
//! trusted local file; plugins receive a permission-projected `PreviewContext`
//! and return a package-confined `PreviewView`.
use crate::plugin_contracts::{
    PreviewContext, PreviewView, ResourceContext, ResourceMetadata, ResourceRef, ScopedRead,
    PREVIEW_CONTEXT_VERSION,
};
use serde_json::Value;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

const SESSION_TTL: Duration = Duration::from_secs(30 * 60);
const MAX_SESSIONS: usize = 64;
const MAX_HTML: u64 = 1024 * 1024;
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
enum Builtin {
    Text,
    Image,
    Media,
    Pdf,
    Fallback,
}

impl Builtin {
    fn id(self) -> &'static str {
        match self {
            Self::Text => "builtin:text",
            Self::Image => "builtin:image",
            Self::Media => "builtin:media",
            Self::Pdf => "builtin:pdf",
            Self::Fallback => "builtin:fallback",
        }
    }

    fn priority(self) -> i64 {
        match self {
            Self::Text => 10,
            Self::Image => 20,
            Self::Media => 30,
            Self::Pdf => 40,
            Self::Fallback => 0,
        }
    }
}

#[derive(Clone)]
struct Provider {
    id: String,
    provider: String,
    priority: i64,
    builtin: Option<Builtin>,
    plugin: Option<(String, String)>,
}

impl Provider {
    fn json(&self) -> Value {
        serde_json::json!({
            "id": self.id,
            "provider": self.provider,
            "priority": self.priority,
        })
    }
}

struct Session {
    plugin_id: String,
    directory: PathBuf,
    path: PathBuf,
    created: Instant,
}

fn sessions() -> &'static Mutex<HashMap<String, Session>> {
    static VALUES: OnceLock<Mutex<HashMap<String, Session>>> = OnceLock::new();
    VALUES.get_or_init(|| Mutex::new(HashMap::new()))
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

fn builtin_candidates(ext: &str, mime: &str, is_dir: bool) -> Vec<Builtin> {
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
    if values.is_empty() {
        values.push(Builtin::Fallback);
    }
    values
}

fn candidates(ext: &str, mime: &str, is_dir: bool) -> Vec<Provider> {
    let ext = ext.trim_start_matches('.').to_ascii_lowercase();
    let mut values = builtin_candidates(&ext, mime, is_dir)
        .into_iter()
        .map(|builtin| Provider {
            id: builtin.id().into(),
            provider: "Fiex".into(),
            priority: builtin.priority(),
            builtin: Some(builtin),
            plugin: None,
        })
        .collect::<Vec<_>>();
    for plugin in crate::plugin_host::catalog() {
        if !plugin.enabled || !plugin.approved || !plugin.backend_available {
            continue;
        }
        let Some(contribution) = plugin.extensions.get("preview") else {
            continue;
        };
        let rules = crate::plugin_match::MatchRules::from_contribution(contribution);
        if !rules.matches(&ext, mime, is_dir) {
            continue;
        }
        values.push(Provider {
            id: format!("plugin:{}", plugin.id),
            provider: plugin.name.clone(),
            priority: contribution
                .get("priority")
                .and_then(Value::as_u64)
                .unwrap_or(50) as i64,
            builtin: None,
            plugin: Some((plugin.id.clone(), plugin.id.clone())),
        });
    }
    values.sort_by_key(|provider| std::cmp::Reverse(provider.priority));
    values
}

/// Ordered provider list for the source switcher. User ordering and the
/// disabled list remain presentation preferences.
pub fn candidates_json(ext: &str, mime: &str, is_dir: bool) -> Vec<Value> {
    candidates(ext, mime, is_dir)
        .iter()
        .map(Provider::json)
        .collect()
}

fn resource_ref(display: &str) -> ResourceRef {
    if crate::mounts::handles(display) {
        if let Ok(uri) = crate::mounts::parse(display) {
            return ResourceRef {
                mount_id: uri.mount_id,
                node_id: format!("fiex-resource:{}", uri.relative),
            };
        }
    }
    ResourceRef {
        mount_id: "local".into(),
        node_id: format!(
            "fiex-resource:{}",
            crate::mounts::new_id().unwrap_or_default()
        ),
    }
}

fn metadata(local: &Path, name: &str) -> Option<ResourceMetadata> {
    let value = fs::symlink_metadata(local).ok()?;
    let extension = Path::new(name)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());
    Some(ResourceMetadata {
        name: name.into(),
        kind: if value.is_dir() { "directory" } else { "file" }.into(),
        mime: extension.as_deref().map(crate::plugin_match::mime_for),
        extension,
        size: (!value.is_dir()).then_some(value.len()),
        modified: value
            .modified()
            .ok()
            .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|value| value.as_secs() as i64),
    })
}

fn builtin_view(kind: Builtin, local: &Path, _display: &str) -> Result<Value, String> {
    let name = name_of(local);
    let ext = extension(local);
    let mime = mime_for(&ext);
    match kind {
        Builtin::Text => {
            let bytes = fs::read(local).map_err(|error| error.to_string())?;
            let truncated = bytes.len() > MAX_TEXT;
            let text = String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_TEXT)]).into_owned();
            Ok(serde_json::json!({
                "kind": "text",
                "text": text,
                "truncated": truncated,
                "size": bytes.len(),
            }))
        }
        Builtin::Image => Ok(serde_json::json!({
            "kind": "image",
            "path": local.to_string_lossy(),
            "name": name,
            "mime": mime,
        })),
        Builtin::Media => Ok(serde_json::json!({
            "kind": "media",
            "media": if mime.starts_with("video/") { "video" } else { "audio" },
            "path": local.to_string_lossy(),
            "name": name,
            "mime": mime,
        })),
        Builtin::Pdf => Ok(serde_json::json!({
            "kind": "pdf",
            "path": local.to_string_lossy(),
            "name": name,
        })),
        Builtin::Fallback => Ok(serde_json::json!({
            "kind": "fallback",
            "name": name,
            "extension": ext,
        })),
    }
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

/// Resolve the provider's package-confined HTML document and validate the
/// returned PreviewView.
fn validate(plugin_id: &str, value: Value) -> Result<Value, String> {
    let view: PreviewView =
        serde_json::from_value(value).map_err(|error| format!("invalid PreviewView: {error}"))?;
    match view.kind.as_str() {
        "html" => {
            let package = view.view.as_ref().ok_or("html preview requires view")?;
            if package.source != "package" {
                return Err("preview view source must be the plugin package".into());
            }
            let path = crate::plugin_host::plugin_view_asset(plugin_id, &package.entry)?;
            let size = fs::metadata(&path)
                .map_err(|error| error.to_string())?
                .len();
            if size > MAX_HTML {
                return Err("preview HTML exceeds size limit".into());
            }
            let html = fs::read_to_string(&path).map_err(|error| error.to_string())?;
            Ok(serde_json::json!({
                "kind": "html",
                "html": html,
                "state": view.state,
                "entry": package.entry,
            }))
        }
        "text" => {
            let text = view.text.unwrap_or_default();
            if text.len() > MAX_TEXT {
                return Err("preview text exceeds size limit".into());
            }
            Ok(serde_json::json!({"kind":"text","text":text}))
        }
        "unsupported" => Ok(serde_json::json!({
            "kind": "fallback",
            "message": view.message.unwrap_or_else(|| "此文件不支持插件预览".into()),
        })),
        other => Err(format!("unsupported PreviewView kind: {other}")),
    }
}

fn open_plugin(
    display: &str,
    local: &str,
    plugin_id: &str,
    contribution_id: &str,
) -> Result<Value, String> {
    let plugin = crate::plugin_host::catalog()
        .into_iter()
        .find(|plugin| plugin.id == plugin_id && plugin.enabled && plugin.approved)
        .ok_or("preview plugin is not active")?;
    let declared = plugin
        .declared_permissions
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let metadata_allowed =
        crate::plugin_permissions::allowed(plugin_id, &declared, "metadata").unwrap_or(false);
    let read_allowed =
        crate::plugin_permissions::allowed(plugin_id, &declared, "files.read").unwrap_or(false);
    let contribution = plugin
        .extensions
        .get("preview")
        .ok_or("plugin does not register a preview provider")?;
    let local_path = Path::new(local);
    let ext = extension(local_path);
    if contribution_id != plugin_id {
        return Err("unknown preview provider".into());
    }
    let rules = crate::plugin_match::MatchRules::from_contribution(contribution);
    if !rules.matches(&ext, &mime_for(&ext), local_path.is_dir()) {
        return Err("preview provider does not match this file type".into());
    }
    let name = name_of(local_path);
    let directory = local_path
        .parent()
        .ok_or("preview file has no parent")?
        .to_path_buf();
    let session_id = crate::mounts::new_id()?;
    let context = PreviewContext {
        version: PREVIEW_CONTEXT_VERSION,
        session_id: session_id.clone(),
        intent: "preview".into(),
        resource: ResourceContext {
            reference: resource_ref(display),
            metadata: metadata_allowed
                .then(|| metadata(local_path, &name))
                .flatten(),
            read: read_allowed.then(|| ScopedRead {
                resource_id: crate::plugin_scoped_reads::register(plugin_id, &session_id, &name),
            }),
        },
    };
    let params = serde_json::to_value(&context).map_err(|error| error.to_string())?;
    let selected = [local_path.to_path_buf()];
    let result = crate::plugin_host::global().invoke_selected(
        plugin_id,
        "preview.open",
        params,
        &directory.to_string_lossy(),
        &selected,
    )?;
    let view = validate(plugin_id, result)?;
    let mut values = sessions()
        .lock()
        .map_err(|_| "preview session registry unavailable")?;
    values.retain(|_, session| session.created.elapsed() < SESSION_TTL);
    if values.len() >= MAX_SESSIONS {
        if let Some(oldest) = values
            .iter()
            .min_by_key(|(_, session)| session.created)
            .map(|(id, _)| id.clone())
        {
            values.remove(&oldest);
        }
    }
    values.insert(
        session_id.clone(),
        Session {
            plugin_id: plugin_id.into(),
            directory,
            path: local_path.to_path_buf(),
            created: Instant::now(),
        },
    );
    Ok(serde_json::json!({
        "sessionId": session_id,
        "pluginId": plugin_id,
        "contributionId": contribution_id,
        "context": context,
        "view": view,
    }))
}

/// Open a provider preview using a unified provider id. `display` is the
/// user-visible URI/path; `local` is the Core-owned file.
pub fn open_provider(
    display: &str,
    local: &str,
    provider_id: Option<&str>,
) -> Result<Value, String> {
    let local_path = Path::new(local);
    let ext = extension(local_path);
    let mime = mime_for(&ext);
    let values = candidates(&ext, &mime, local_path.is_dir());
    let provider = match provider_id {
        Some(id) => values
            .iter()
            .find(|provider| provider.id == id)
            .cloned()
            .ok_or("unknown preview provider")?,
        None => values.first().cloned().ok_or("no preview provider")?,
    };
    let view = if let Some(builtin) = provider.builtin {
        builtin_view(builtin, local_path, display)?
    } else if let Some((plugin_id, contribution_id)) = provider.plugin.as_ref() {
        let plugin = open_plugin(display, local, plugin_id, contribution_id)?;
        let mut view = plugin
            .get("view")
            .cloned()
            .ok_or("plugin preview response missing view")?;
        if let Some(object) = view.as_object_mut() {
            object.insert("providerId".into(), Value::String(provider.id.clone()));
            object.insert("provider".into(), Value::String(provider.provider.clone()));
            if let Some(session) = plugin.get("sessionId") {
                object.insert("sessionId".into(), session.clone());
            }
            if let Some(context) = plugin.get("context") {
                object.insert("context".into(), context.clone());
            }
            if let Some(plugin_id) = plugin.get("pluginId") {
                object.insert("pluginId".into(), plugin_id.clone());
            }
            if let Some(contribution_id) = plugin.get("contributionId") {
                object.insert("contributionId".into(), contribution_id.clone());
            }
        }
        view
    } else {
        return Err("invalid preview provider".into());
    };
    Ok(serde_json::json!({
        "providerId": provider.id,
        "provider": provider.provider,
        "priority": provider.priority,
        "view": view,
    }))
}

/// Compatibility wrapper for the plugin-only integration test.
pub fn open(
    display: &str,
    local: &str,
    plugin_id: &str,
    contribution_id: &str,
) -> Result<Value, String> {
    let result = open_plugin(display, local, plugin_id, contribution_id)?;
    Ok(serde_json::json!({
        "sessionId": result["sessionId"],
        "pluginId": plugin_id,
        "contributionId": contribution_id,
        "context": result["context"],
        "view": result["view"],
    }))
}

/// Resolve the operation scope for a plugin UI invoking its backend. The
/// iframe only holds the opaque session id, never the selected path.
pub fn session_scope(session_id: &str, plugin_id: &str) -> Result<(String, Vec<PathBuf>), String> {
    let mut values = sessions()
        .lock()
        .map_err(|_| "preview session registry unavailable")?;
    values.retain(|_, session| session.created.elapsed() < SESSION_TTL);
    let session = values.get(session_id).ok_or("preview session expired")?;
    if session.plugin_id != plugin_id {
        return Err("preview session belongs to another plugin".into());
    }
    Ok((
        session.directory.to_string_lossy().into_owned(),
        vec![session.path.clone()],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_preview_kind_is_rejected() {
        let error = validate(
            "org.fiex.test",
            serde_json::json!({"kind":"iframe","view":{"source":"package","entry":"ui/x.html"}}),
        )
        .unwrap_err();
        assert!(error.contains("invalid PreviewView") || error.contains("unsupported"));
    }

    #[test]
    fn builtins_are_ordered_and_fallback_is_last() {
        let values = candidates("md", "text/markdown", false);
        assert_eq!(values[0].id, "builtin:text");
        assert!(!values
            .iter()
            .any(|provider| provider.id == "builtin:fallback"));
        let pdf = candidates("pdf", "application/pdf", false);
        assert!(pdf.iter().any(|provider| provider.id == "builtin:pdf"));
        let unknown = candidates("xyz", "application/octet-stream", false);
        assert_eq!(unknown.len(), 1);
        assert_eq!(unknown[0].id, "builtin:fallback");
    }
}
