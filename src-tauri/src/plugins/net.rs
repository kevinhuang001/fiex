//! Host-mediated HTTP and TCP access for plugins.
//!
//! Plugins never open sockets. HTTP is a structured request/response API;
//! TCP is a controlled byte stream sharing HTTP's approved hostname/IP list.
#[path = "net/tcp.rs"]
mod tcp;
use crate::plugin_permission_policy::PermissionPolicy;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
pub use tcp::{cleanup, tcp_close, tcp_open, tcp_read_bytes, tcp_shutdown, tcp_write_bytes};

fn text_param<'a>(p: &'a Value, key: &str) -> Result<&'a str, String> {
    p.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing {key}"))
}

fn http_method(value: &str) -> bool {
    matches!(
        value.to_ascii_uppercase().as_str(),
        "GET"
            | "HEAD"
            | "POST"
            | "PUT"
            | "DELETE"
            | "PATCH"
            | "OPTIONS"
            | "PROPFIND"
            | "PROPPATCH"
            | "MKCOL"
            | "MOVE"
            | "COPY"
    )
}

#[derive(Clone)]
struct HttpEntry {
    owner: String,
    response: std::sync::Arc<Mutex<reqwest::blocking::Response>>,
}
fn http_streams() -> &'static Mutex<HashMap<String, HttpEntry>> {
    static STREAMS: OnceLock<Mutex<HashMap<String, HttpEntry>>> = OnceLock::new();
    STREAMS.get_or_init(|| Mutex::new(HashMap::new()))
}
pub fn http_open(
    owner: &str,
    policy: &PermissionPolicy<'_>,
    params: &Value,
) -> Result<Value, String> {
    let mut url = reqwest::Url::parse(text_param(params, "url")?).map_err(|e| e.to_string())?;
    let mut method = params
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or("GET")
        .to_ascii_uppercase();
    if !http_method(&method) {
        return Err("unsupported HTTP method".into());
    }
    let timeout = params
        .get("timeoutMs")
        .and_then(Value::as_u64)
        .unwrap_or(90000)
        .clamp(
            1,
            if params
                .get("bodyReadHandle")
                .and_then(Value::as_str)
                .is_some()
            {
                86_400_000
            } else {
                120_000
            },
        );
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(timeout))
        .redirect(reqwest::redirect::Policy::none())
        .danger_accept_invalid_certs(
            params
                .get("allowInvalidCerts")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        )
        .build()
        .map_err(|e| e.to_string())?;
    let mut headers = reqwest::header::HeaderMap::new();
    if let Some(values) = params.get("headers").and_then(Value::as_object) {
        if values.len() > 64 {
            return Err("too many HTTP headers".into());
        }
        for (key, value) in values {
            let value = value.as_str().ok_or("header must be a string")?;
            if value.len() > 8192 {
                return Err("HTTP header too large".into());
            }
            headers.insert(
                reqwest::header::HeaderName::from_bytes(key.as_bytes())
                    .map_err(|e| e.to_string())?,
                reqwest::header::HeaderValue::from_str(value).map_err(|e| e.to_string())?,
            );
        }
    }
    use base64::Engine;
    let mut body = params
        .get("bodyBase64")
        .and_then(Value::as_str)
        .map(|s| {
            base64::engine::general_purpose::STANDARD
                .decode(s)
                .map_err(|e| e.to_string())
        })
        .transpose()?;
    if body.as_ref().is_some_and(|b| b.len() > 1024 * 1024) {
        return Err("HTTP request exceeds inline limit".into());
    }
    let mut file_body = params
        .get("bodyReadHandle")
        .and_then(Value::as_str)
        .map(|id| crate::plugin_fs_access::http_body(owner, id))
        .transpose()?;
    if file_body.is_some() && body.as_ref().is_some_and(|b| !b.is_empty()) {
        return Err("ambiguous HTTP body".into());
    }
    for redirect in 0..=10 {
        if !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err("invalid HTTP URL".into());
        }
        if !policy.domain_allowed(url.host_str().ok_or("missing HTTP host")?)? {
            return Err("network target not granted".into());
        }
        let mut request = client
            .request(
                reqwest::Method::from_bytes(method.as_bytes()).map_err(|e| e.to_string())?,
                url.clone(),
            )
            .headers(headers.clone());
        if let Some(file) = &mut file_body {
            use std::io::{Seek, SeekFrom};
            file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
            let size = file.metadata().map_err(|e| e.to_string())?.len();
            request = request.body(reqwest::blocking::Body::sized(
                file.try_clone().map_err(|e| e.to_string())?,
                size,
            ));
        } else if let Some(bytes) = &body {
            request = request.body(bytes.clone());
        }
        let response = request.send().map_err(|e| e.to_string())?;
        // Google resumable uploads use 308 without Location to acknowledge
        // an incomplete upload. It is a response, not a redirect.
        if matches!(response.status().as_u16(), 301 | 302 | 303 | 307 | 308)
            && (response.status().as_u16() != 308
                || response.headers().contains_key(reqwest::header::LOCATION))
        {
            if redirect == 10 {
                return Err("too many HTTP redirects".into());
            }
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .ok_or("redirect missing Location")?
                .to_str()
                .map_err(|e| e.to_string())?;
            let next = url.join(location).map_err(|e| e.to_string())?;
            if url.scheme() == "https" && next.scheme() == "http" {
                return Err("HTTPS downgrade redirect rejected".into());
            }
            if url.origin() != next.origin() {
                headers.remove(reqwest::header::AUTHORIZATION);
                headers.remove(reqwest::header::COOKIE);
            }
            if response.status().as_u16() == 303
                || (matches!(response.status().as_u16(), 301 | 302) && method == "POST")
            {
                method = "GET".into();
                body = None;
                file_body = None;
                headers.remove(reqwest::header::CONTENT_TYPE);
            }
            url = next;
            continue;
        }
        let status = response.status().as_u16();
        let headers: BTreeMap<_, _> = response
            .headers()
            .iter()
            .filter_map(|(k, v)| {
                v.to_str()
                    .ok()
                    .map(|v| (k.as_str().to_owned(), v.to_owned()))
            })
            .collect();
        let mut entries = http_streams()
            .lock()
            .map_err(|_| "HTTP registry unavailable")?;
        if entries
            .values()
            .filter(|entry| entry.owner == owner)
            .count()
            >= 16
        {
            return Err("too many HTTP responses".into());
        }
        let id = format!(
            "http-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_nanos()
        );
        entries.insert(
            id.clone(),
            HttpEntry {
                owner: owner.into(),
                response: std::sync::Arc::new(Mutex::new(response)),
            },
        );
        return Ok(json!({"stream": id, "status": status, "headers": headers}));
    }
    Err("HTTP redirect failed".into())
}
pub fn http_read(owner: &str, params: &Value) -> Result<Value, String> {
    let response = http_streams()
        .lock()
        .map_err(|_| "HTTP registry unavailable")?
        .get(text_param(params, "stream")?)
        .filter(|entry| entry.owner == owner)
        .map(|entry| entry.response.clone())
        .ok_or("HTTP response unavailable")?;
    let size = params
        .get("size")
        .and_then(Value::as_u64)
        .unwrap_or(65536)
        .clamp(1, 262144) as usize;
    let mut bytes = vec![0; size];
    let read = response
        .lock()
        .map_err(|_| "HTTP response unavailable")?
        .read(&mut bytes)
        .map_err(|e| e.to_string())?;
    bytes.truncate(read);
    use base64::Engine;
    Ok(
        json!({"base64":base64::engine::general_purpose::STANDARD.encode(bytes),"size":read,"eof":read==0}),
    )
}
pub fn http_close(owner: &str, params: &Value) -> Result<(), String> {
    let id = text_param(params, "stream")?;
    let mut entries = http_streams()
        .lock()
        .map_err(|_| "HTTP registry unavailable")?;
    if entries.get(id).is_none_or(|entry| entry.owner != owner) {
        return Err("HTTP response unavailable".into());
    }
    entries.remove(id);
    Ok(())
}
