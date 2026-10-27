//! Unified preview registry: built-ins and plugins return the same resolved view.
//!
//! Providers are matched by extension/MIME and priority. Built-ins use a
//! trusted local file; plugins receive a permission-projected `EventContext`
//! and return a package-confined `PreviewView`.
use crate::plugin_contracts::PreviewView;
use crate::plugin_resource_context::ResourceSnapshot;
use serde_json::Value;
use std::{
    collections::HashMap,
    fs,
    path::Path,
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

use crate::plugin_events::{EventRouter, Provider};

struct Session {
    plugin_id: String,
    resource: ResourceSnapshot,
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
    let builtin = builtin_candidates(&ext, mime, is_dir)
        .into_iter()
        .map(|b| Provider::builtin(b.id(), b.priority()))
        .collect();
    EventRouter::providers(
        "preview.open",
        &[
            serde_json::json!({"extension":ext,"mime":mime,"kind":if is_dir{"directory"}else{"file"}}),
        ],
        "",
        builtin,
    )
}

/// Ordered provider list for the source switcher. User ordering and the
/// disabled list remain presentation preferences. A plugin that declares a
/// loading page advertises it here so the UI can resolve the document before
/// invoking `preview.open`.
pub fn candidates_json(ext: &str, mime: &str, is_dir: bool) -> Vec<Value> {
    candidates(ext, mime, is_dir)
        .iter()
        .map(|provider| {
            let mut value = provider.json();
            if let crate::plugin_events::Handler::Plugin(plugin) = &provider.handler {
                if crate::plugin_host::preview_loading_entry(&plugin.extensions).is_some() {
                    value["loading"] = Value::Bool(true);
                }
            }
            value
        })
        .collect()
}

fn builtin_view(kind: Builtin, local: &Path, _display: &str) -> Result<Value, String> {
    let cache_key = (kind == Builtin::Text)
        .then(|| crate::plugin_cache::key("preview.text", local))
        .flatten();
    if let Some(value) = cache_key.as_deref().and_then(crate::plugin_cache::get) {
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
            Ok(serde_json::json!({
                "kind": "text",
                "text": text,
                "truncated": truncated,
                "size": size,
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
    }?;
    if let Some(key) = cache_key {
        crate::plugin_cache::put(key, view.clone());
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
        "unsupported" => Err("provider does not support this resource".into()),
        other => Err(format!("unsupported PreviewView kind: {other}")),
    }
}

fn open_plugin_resource(
    resource: &ResourceSnapshot,
    plugin: &crate::plugin_host::PluginInfo,
) -> Result<Value, String> {
    let plugin_id = &plugin.id;
    let session_id = crate::mounts::new_id()?;
    let (context, scope) = crate::plugin_events::resource_scope(
        plugin,
        "preview.open",
        serde_json::json!({"sessionId":session_id}),
        std::slice::from_ref(resource),
        None,
    )?;
    let result =
        crate::plugin_events::invoke_prepared(plugin_id, "preview.open", context.clone(), scope)?;
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
            plugin_id: plugin_id.clone(),
            resource: resource.clone(),
            created: Instant::now(),
        },
    );
    Ok(serde_json::json!({
        "sessionId": session_id,
        "pluginId": plugin_id,
        "contributionId": plugin_id,
        // Explicit manifest contract: the preview document may take longer to
        // handshake or answer than a generic panel (large archive listings).
        "timeoutMs": plugin.ui_timeout_ms,
        "context": context,
        "view": view,
    }))
}

/// Open a provider preview using a unified provider id. `display` is the
/// user-visible URI/path; `local` is the Core-owned file.
pub fn open_provider(display: &str, local: &str, preferred: Option<&str>) -> Result<Value, String> {
    resolve_preview(
        ResourceSnapshot::new(display, Path::new(local)),
        preferred,
        None,
    )
}
pub fn open_resource(
    display: &str,
    preferred: Option<&str>,
    order: Option<&[String]>,
) -> Result<Value, String> {
    resolve_preview(ResourceSnapshot::resolve(display)?, preferred, order)
}
fn resolve_preview(
    resource: ResourceSnapshot,
    preferred: Option<&str>,
    order: Option<&[String]>,
) -> Result<Value, String> {
    let mut providers = candidates(&resource.ext, &resource.mime, resource.is_dir);
    if let Some(order) = order {
        providers.retain(|p| order.contains(&p.id));
        providers.sort_by_key(|p| {
            order
                .iter()
                .position(|id| id == &p.id)
                .unwrap_or(usize::MAX)
        });
    }
    if let Some(id) = preferred {
        let index = providers
            .iter()
            .position(|p| p.id == id)
            .ok_or("unknown preview provider")?;
        let first = providers.remove(index);
        providers.insert(0, first);
    }
    // The final fallback is a label-only renderer; it never reads/downloads content.
    providers.retain(|p| p.id != "builtin:fallback");
    providers.push(Provider::builtin("builtin:fallback", i64::MIN));
    EventRouter::resolve(
        "preview.open",
        providers,
        |_, id| {
            let kind = [
                Builtin::Text,
                Builtin::Image,
                Builtin::Media,
                Builtin::Pdf,
                Builtin::Fallback,
            ]
            .into_iter()
            .find(|b| b.id() == id)
            .ok_or("unknown preview builtin")?;
            let path = if kind == Builtin::Fallback {
                resource.local.clone()
            } else {
                resource.materialize()?
            };
            builtin_view(kind, &path, &resource.display)
        },
        |plugin| {
            open_plugin_resource(&resource, plugin).map(|response| {
                let mut view = response["view"].clone();
                if let Some(object) = view.as_object_mut() {
                    for key in [
                        "sessionId",
                        "context",
                        "pluginId",
                        "contributionId",
                        "timeoutMs",
                    ] {
                        object.insert(key.into(), response[key].clone());
                    }
                }
                view
            })
        },
        |provider, mut view| {
            if let Some(object) = view.as_object_mut() {
                object.insert("providerId".into(), Value::String(provider.id.clone()));
                object.insert("provider".into(), Value::String(provider.name.clone()));
            }
            Ok(view)
        },
        |_| true,
        |outcomes| {
            outcomes.into_iter().find_map(|(provider,result)|result.ok().map(|view|serde_json::json!({"providerId":provider.id,"provider":provider.name,"priority":provider.priority,"view":view}))).ok_or_else(||"no preview provider succeeded".into())
        },
    )
}

/// Open an explicitly selected plugin through the same resource broker.
pub fn open(
    display: &str,
    local: &str,
    plugin_id: &str,
    contribution_id: &str,
) -> Result<Value, String> {
    if plugin_id != contribution_id {
        return Err("unknown preview provider".into());
    }
    let id = format!("plugin:{plugin_id}");
    let result = resolve_preview(
        ResourceSnapshot::new(display, Path::new(local)),
        Some(&id),
        Some(std::slice::from_ref(&id)),
    )?;
    if result["providerId"] != id {
        return Err("preview provider does not support this resource".into());
    }
    let view = &result["view"];
    Ok(
        serde_json::json!({"sessionId":view["sessionId"],"pluginId":plugin_id,"contributionId":contribution_id,"context":view["context"],"view":view}),
    )
}

/// Resolve the operation scope for a plugin UI invoking its backend. The
/// iframe only holds the opaque session id, never the selected path.
pub fn session_scope(
    session_id: &str,
    plugin_id: &str,
) -> Result<crate::plugin_fs_access::Scope, String> {
    let mut values = sessions()
        .lock()
        .map_err(|_| "preview session registry unavailable")?;
    values.retain(|_, session| session.created.elapsed() < SESSION_TTL);
    let session = values.get(session_id).ok_or("preview session expired")?;
    if session.plugin_id != plugin_id {
        return Err("preview session belongs to another plugin".into());
    }
    let plugin = crate::plugin_host::catalog()
        .into_iter()
        .find(|p| p.id == plugin_id && p.enabled && p.approved && p.backend_available)
        .ok_or("preview provider unavailable")?;
    let (_, scope) = crate::plugin_events::resource_scope(
        &plugin,
        "preview.open",
        serde_json::json!({}),
        std::slice::from_ref(&session.resource),
        None,
    )?;
    Ok(scope)
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
