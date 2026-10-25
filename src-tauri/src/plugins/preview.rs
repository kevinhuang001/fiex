//! Unified preview registry: built-ins and plugins return the same resolved view.
//!
//! Providers are matched by extension/MIME and priority. Built-ins use a
//! trusted local file; plugins receive a permission-projected event request
//! and return a package-confined `PreviewView`.
use crate::plugin_contracts::PreviewView;
use crate::plugin_resource_broker::Resource;
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

use crate::plugin_events::{self as events, Provider};

struct Session {
    plugin_id: String,
    directory: PathBuf,
    path: PathBuf,
    reference: crate::plugin_contracts::ResourceRef,
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
    values.push(Builtin::Fallback);
    values
}

fn candidates(ext: &str, mime: &str, is_dir: bool) -> Vec<Provider> {
    let ext = ext.trim_start_matches('.').to_ascii_lowercase();
    let builtin = builtin_candidates(&ext, mime, is_dir)
        .into_iter()
        .map(|kind| Provider::builtin(kind.id(), kind.priority()))
        .collect();
    events::registry("preview", &ext, mime, is_dir, builtin)
}

/// Ordered provider list for the source switcher. User ordering and the
/// disabled list remain presentation preferences.
pub fn candidates_json(ext: &str, mime: &str, is_dir: bool) -> Vec<Value> {
    candidates(ext, mime, is_dir)
        .iter()
        .map(Provider::json)
        .collect()
}

fn builtin_view(kind: Builtin, local: &Path, _display: &str) -> Result<Value, String> {
    let name = name_of(local);
    let ext = extension(local);
    let mime = mime_for(&ext);
    match kind {
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
    let policy = crate::plugin_permission_policy::PermissionPolicy::new(plugin_id, &declared);
    let metadata_allowed = policy.allowed(crate::plugin_permission_policy::Capability::Metadata);
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
    let directory = local_path
        .parent()
        .ok_or("preview file has no parent")?
        .to_path_buf();
    let session_id = crate::mounts::new_id()?;
    let resource = Resource::new(display, local_path);
    let context = resource.request(metadata_allowed, serde_json::json!({}));
    let result =
        crate::plugin_events::invoke(plugin_id, "preview.open", context.clone(), local_path)?;
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
            reference: resource.reference,
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
    open_provider_ordered(display, local, provider_id, None)
}

pub fn open_provider_ordered(
    display: &str,
    local: &str,
    provider_id: Option<&str>,
    provider_order: Option<&[String]>,
) -> Result<Value, String> {
    let local_path = Path::new(local);
    let ext = extension(local_path);
    let mime = mime_for(&ext);
    let values = candidates(&ext, &mime, local_path.is_dir());
    let mut values = values;
    if let Some(order) = provider_order {
        values.retain(|p| order.contains(&p.id) || p.id == "builtin:fallback");
        values.sort_by_key(|p| {
            order
                .iter()
                .position(|id| id == &p.id)
                .unwrap_or(usize::MAX)
        });
    }
    if let Some(id) = provider_id {
        let index = values
            .iter()
            .position(|provider| provider.id == id)
            .ok_or("unknown preview provider")?;
        let preferred = values.remove(index);
        values.insert(0, preferred);
    }
    let results = events::EventRouter::resolve(
        "preview.open",
        values,
        |_, id| {
            let kind = builtin_candidates(&ext, &mime, local_path.is_dir())
                .into_iter()
                .find(|kind| kind.id() == id)
                .ok_or("unknown builtin preview")?;
            builtin_view(kind, local_path, display)
        },
        |plugin| {
            let plugin = open_plugin(display, local, &plugin.id, &plugin.id)?;
            let mut view = plugin
                .get("view")
                .cloned()
                .ok_or("plugin preview response missing view")?;
            if let Some(object) = view.as_object_mut() {
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
            Ok(view)
        },
        |_, view| {
            if !matches!(
                view["kind"].as_str(),
                Some("html" | "text" | "image" | "media" | "pdf" | "fallback")
            ) {
                return Err("invalid preview result kind".into());
            }
            Ok(view)
        },
        |view| view["kind"] != "fallback",
    );
    let mut fallback = None;
    for (provider, result) in results {
        if let Ok(mut view) = result {
            if let Some(object) = view.as_object_mut() {
                object.insert("providerId".into(), Value::String(provider.id.clone()));
                object.insert("provider".into(), Value::String(provider.name.clone()));
            }
            return Ok(
                serde_json::json!({"providerId":provider.id,"provider":provider.name,"priority":provider.priority,"view":view}),
            );
        }
        if provider.id == "builtin:fallback" {
            fallback = Some(provider);
        }
    }
    let provider = fallback.unwrap_or_else(|| Provider::builtin("builtin:fallback", 0));
    let view = builtin_view(Builtin::Fallback, local_path, display)?;
    Ok(serde_json::json!({
        "providerId": provider.id,
        "provider": provider.name,
        "priority": provider.priority,
        "view": view,
    }))
}

/// Open a specific plugin provider.
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
    let mut scope =
        crate::plugin_fs_access::Scope::selected(&session.directory, &[session.path.clone()])?;
    scope.bind_reference(&session.reference, &session.path)?;
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
        assert_eq!(values.last().unwrap().id, "builtin:fallback");
        let pdf = candidates("pdf", "application/pdf", false);
        assert!(pdf.iter().any(|provider| provider.id == "builtin:pdf"));
        let unknown = candidates("xyz", "application/octet-stream", false);
        assert_eq!(unknown.len(), 1);
        assert_eq!(unknown[0].id, "builtin:fallback");
    }
}
