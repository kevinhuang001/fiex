use super::clean;

/// The SDK emits a payload-free request line. Core already records the same
/// request with selected parameters, so omit the duplicate lifecycle entry.
pub fn is_sdk_request_lifecycle(message: &str) -> bool {
    let Some((method, rest)) = message.split_once(' ') else {
        return false;
    };
    !method.is_empty()
        && method.contains('.')
        && !method.chars().any(char::is_whitespace)
        && rest.contains("duration_ms=")
        && (rest.starts_with("completed")
            || rest.starts_with("failed")
            || rest.starts_with("expected outcome"))
}

/// Whitelisted RPC parameters that are useful in a log entry. Unknown keys
/// are omitted on purpose: plugin calls may carry credentials or binary data.
const IMPORTANT_PARAMS: &[&str] = &[
    "nodeId",
    "parentId",
    "path",
    "name",
    "query",
    "mode",
    "hidden",
    "limit",
    "cursor",
    "offset",
    "maxBytes",
    "size",
    "overwrite",
    "streamId",
    "direction",
    "action",
    "operations",
    "ignores",
    "reference",
    "root",
    "branch",
    "commit",
    "id",
    "kind",
    "extension",
    "mime",
    "command",
];

fn sensitive(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "token",
        "secret",
        "password",
        "passwd",
        "credential",
        "authorization",
        "cookie",
        "apikey",
        "api_key",
        "base64",
        "content",
        "payload",
        "bytes",
    ]
    .iter()
    .any(|part| key.contains(part))
}

/// Compact, credential-free parameter summary for request lifecycle entries.
pub fn request_detail(_method: &str, params: &serde_json::Value) -> String {
    let Some(object) = params.as_object() else {
        return String::new();
    };
    let mut parts = Vec::new();
    for (key, value) in object {
        if sensitive(key) || !IMPORTANT_PARAMS.contains(&key.as_str()) {
            continue;
        }
        match value {
            serde_json::Value::Null => {}
            serde_json::Value::Bool(value) => parts.push(format!("{key}={value}")),
            serde_json::Value::Number(value) => parts.push(format!("{key}={value}")),
            serde_json::Value::String(value) => {
                parts.push(format!("{key}=\"{}\"", clean(value, 200)));
            }
            serde_json::Value::Array(values) => {
                if key == "operations" {
                    let list = values
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .take(16)
                        .collect::<Vec<_>>()
                        .join(",");
                    parts.push(format!("{key}=[{list}]"));
                } else {
                    parts.push(format!("{key}=[{} items]", values.len()));
                }
            }
            serde_json::Value::Object(_) => {
                if key == "reference" {
                    let mount = value
                        .get("mountId")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("");
                    let id = value
                        .get("id")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("");
                    if !mount.is_empty() || !id.is_empty() {
                        parts.push(format!(
                            "reference={}",
                            clean(&format!("{mount}/{id}"), 160)
                        ));
                    }
                } else {
                    parts.push(format!("{key}={{…}}"));
                }
            }
        }
    }
    parts.join("  ")
}

/// Result highlights worth keeping next to a completed request. Payloads and
/// entry lists stay in the response; only counts and small scalars are logged.
pub fn response_detail(value: &serde_json::Value) -> String {
    if let Some(object) = value.as_object() {
        let mut parts = Vec::new();
        if let Some(entries) = object.get("entries").and_then(serde_json::Value::as_array) {
            parts.push(format!("entries={}", entries.len()));
        }
        if let Some(size) = object.get("size") {
            parts.push(format!("size={size}"));
        }
        if let Some(truncated) = object.get("truncated").and_then(serde_json::Value::as_bool) {
            parts.push(format!("truncated={truncated}"));
        }
        if object
            .get("nextCursor")
            .is_some_and(|value| !value.is_null())
        {
            parts.push("nextCursor=yes".into());
        }
        if let Some(name) = object.get("name").and_then(serde_json::Value::as_str) {
            parts.push(format!("name=\"{}\"", clean(name, 120)));
        }
        return parts.join("  ");
    }
    if let Some(items) = value.as_array() {
        return format!("items={}", items.len());
    }
    String::new()
}
