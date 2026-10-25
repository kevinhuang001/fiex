//! Standard `preview.open` registry.
//!
//! Providers return a `PreviewView` after Core projects an authorized
//! `PreviewContext`. The provider never receives a host path; it reads the
//! selected file through the scoped operation or the `read` capability.
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

fn matches(contribution: &Value, ext: &str) -> bool {
    let Some(values) = contribution.get("extensions").and_then(Value::as_array) else {
        return true;
    };
    if values.is_empty() {
        return true;
    }
    values.iter().filter_map(Value::as_str).any(|value| {
        let value = value
            .trim_start_matches("*.")
            .trim_start_matches('.')
            .to_ascii_lowercase();
        value == "*" || value == ext
    })
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
    Some(ResourceMetadata {
        name: name.into(),
        kind: if value.is_dir() { "directory" } else { "file" }.into(),
        extension: Path::new(name)
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase()),
        size: (!value.is_dir()).then_some(value.len()),
        modified: value
            .modified()
            .ok()
            .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|value| value.as_secs() as i64),
    })
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
                "title": view.title,
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
            Ok(serde_json::json!({"kind":"text","title":view.title,"text":text}))
        }
        "unsupported" => Ok(serde_json::json!({
            "kind": "unsupported",
            "title": view.title,
            "message": view.message.unwrap_or_else(|| "此文件不支持插件预览".into()),
        })),
        other => Err(format!("unsupported PreviewView kind: {other}")),
    }
}

/// Ordered preview providers for an extension. User ordering and the
/// enabled/disabled list remain presentation preferences; Core owns matching,
/// permission checks and priority.
pub fn providers(extension: &str) -> Vec<Value> {
    let ext = extension.trim_start_matches('.').to_ascii_lowercase();
    let mut values = Vec::new();
    for plugin in crate::plugin_host::catalog() {
        if !plugin.enabled || !plugin.approved || !plugin.backend_available {
            continue;
        }
        let Some(contributions) = plugin.extensions.get("preview").and_then(Value::as_array) else {
            continue;
        };
        for contribution in contributions {
            if !matches(contribution, &ext) {
                continue;
            }
            let Some(id) = contribution.get("id").and_then(Value::as_str) else {
                continue;
            };
            values.push(serde_json::json!({
                "pluginId": plugin.id,
                "contributionId": id,
                "title": contribution.get("title").and_then(Value::as_str).unwrap_or(&plugin.name),
                "priority": contribution.get("priority").and_then(Value::as_u64).unwrap_or(50),
            }));
        }
    }
    values.sort_by_key(|value| std::cmp::Reverse(value["priority"].as_u64().unwrap_or(0)));
    values
}

/// Open a provider preview. `display` is the user-visible URI/path; `local` is
/// the Core-owned file used for scoped reads and metadata.
pub fn open(
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
    let contributions = plugin
        .extensions
        .get("preview")
        .and_then(Value::as_array)
        .ok_or("plugin does not register preview providers")?;
    let local_path = Path::new(local);
    let ext = extension(local_path);
    let contribution = contributions
        .iter()
        .find(|entry| entry.get("id").and_then(Value::as_str) == Some(contribution_id))
        .ok_or("unknown preview provider")?;
    if !matches(contribution, &ext) {
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
    fn extension_matching_accepts_wildcards() {
        assert!(matches(&serde_json::json!({}), "zip"));
        assert!(matches(
            &serde_json::json!({"extensions":["*.ZIP","rar"]}),
            "zip"
        ));
        assert!(!matches(&serde_json::json!({"extensions":["zip"]}), "rar"));
    }
}
