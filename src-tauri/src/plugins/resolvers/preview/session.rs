use super::*;
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
const SESSION_TTL: Duration = Duration::from_secs(30 * 60);
const MAX_SESSIONS: usize = 64;
const MAX_HTML: u64 = 1024 * 1024;
const MAX_TEXT: usize = 256 * 1024;
/// A provider saying "this resource is not mine" is not a failure: the router
/// may fall through to the next provider. Any other error keeps the user's
/// selected provider and is reported instead of being masked by the builtin.
pub(super) const UNSUPPORTED: &str = "fiex.preview.unsupported";
struct Session {
    plugin_id: String,
    resource: ResourceSnapshot,
    created: Instant,
}

fn sessions() -> &'static Mutex<HashMap<String, Session>> {
    static VALUES: OnceLock<Mutex<HashMap<String, Session>>> = OnceLock::new();
    VALUES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Resolve the provider's package-confined HTML document and validate the
/// returned PreviewView.
pub(super) fn validate(plugin_id: &str, value: Value) -> Result<Value, String> {
    let view: PreviewView =
        serde_json::from_value(value).map_err(|error| format!("invalid PreviewView: {error}"))?;
    match view.kind.as_str() {
        "html" => {
            let package = view.view.as_ref().ok_or("html preview requires view")?;
            if package.source != "package" {
                return Err("preview view source must be the plugin package".into());
            }
            let path = crate::plugins::host::plugin_view_asset(plugin_id, &package.entry)?;
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
        "unsupported" => Err(UNSUPPORTED.into()),
        other => Err(format!("unsupported PreviewView kind: {other}")),
    }
}

pub(super) fn open_plugin_resource(
    resource: &ResourceSnapshot,
    plugin: &crate::plugins::host::PluginInfo,
) -> Result<Value, String> {
    let plugin_id = &plugin.id;
    let session_id = crate::plugins::mounts::new_id()?;
    let (context, scope) = crate::plugins::events::resource_scope(
        plugin,
        "preview.open",
        serde_json::json!({"sessionId":session_id}),
        std::slice::from_ref(resource),
        None,
    )?;
    let result =
        crate::plugins::events::invoke_prepared(plugin_id, "preview.open", context.clone(), scope)?;
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

/// Resolve the operation scope for a plugin UI invoking its backend. The
/// iframe only holds the opaque session id, never the selected path.
pub fn session_scope(
    session_id: &str,
    plugin_id: &str,
) -> Result<crate::plugins::resource::fs_access::Scope, String> {
    let mut values = sessions()
        .lock()
        .map_err(|_| "preview session registry unavailable")?;
    values.retain(|_, session| session.created.elapsed() < SESSION_TTL);
    let session = values.get(session_id).ok_or("preview session expired")?;
    if session.plugin_id != plugin_id {
        return Err("preview session belongs to another plugin".into());
    }
    let plugin = crate::plugins::host::catalog()
        .into_iter()
        .find(|p| p.id == plugin_id && p.enabled && p.approved && p.backend_available)
        .ok_or("preview provider unavailable")?;
    let (_, scope) = crate::plugins::events::resource_scope(
        &plugin,
        "preview.open",
        serde_json::json!({}),
        std::slice::from_ref(&session.resource),
        None,
    )?;
    Ok(scope)
}
