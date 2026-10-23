//! Host-mediated HTTP and TCP access for plugins.
//!
//! Plugins never open sockets. HTTP is a structured request/response API;
//! TCP is a controlled byte stream limited to manifest-declared host/port
//! targets.
use crate::{plugin_ipc, plugin_permissions};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const MAX_HTTP_BODY: usize = 64 * 1024 * 1024;
const MAX_TCP_CHUNK: usize = 8 * 1024 * 1024;

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

pub fn http(
    owner: &str,
    declared: &std::collections::BTreeSet<String>,
    params: &Value,
) -> Result<Value, String> {
    let url = text_param(params, "url")?;
    let method = text_param(params, "method")?.to_ascii_uppercase();
    if !http_method(&method) {
        return Err("unsupported HTTP method".into());
    }
    let parsed = reqwest::Url::parse(url).map_err(|error| error.to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("only http and https are supported".into());
    }
    let host = parsed.host_str().ok_or("URL has no host")?;
    if !plugin_permissions::domain_allowed(owner, declared, host)? {
        return Err("network domain not granted".into());
    }
    let mut headers = reqwest::header::HeaderMap::new();
    if let Some(values) = params.get("headers").and_then(Value::as_object) {
        for (key, value) in values.iter().take(64) {
            let name = reqwest::header::HeaderName::from_bytes(key.as_bytes())
                .map_err(|error| error.to_string())?;
            let value = value.as_str().ok_or("header value must be a string")?;
            if value.len() > 8192 {
                return Err("header value too large".into());
            }
            headers.append(
                name,
                reqwest::header::HeaderValue::from_str(value).map_err(|error| error.to_string())?,
            );
        }
    }
    let body = match params.get("bodyBuffer").and_then(Value::as_str) {
        Some(buffer) => Some(plugin_ipc::read(owner, buffer)?),
        None => None,
    };
    if body
        .as_ref()
        .is_some_and(|value| value.len() > MAX_HTTP_BODY)
    {
        return Err("request body too large".into());
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| error.to_string())?;
    let mut request = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).map_err(|error| error.to_string())?,
            parsed,
        )
        .headers(headers);
    if let Some(body) = body {
        request = request.body(body);
    }
    let response = request.send().map_err(|error| error.to_string())?;
    let status = response.status().as_u16();
    let mut response_headers = BTreeMap::new();
    for (name, value) in response.headers().iter().take(128) {
        if let Ok(value) = value.to_str() {
            response_headers.insert(name.as_str().to_owned(), value.to_owned());
        }
    }
    let bytes = response.bytes().map_err(|error| error.to_string())?;
    if bytes.len() > MAX_HTTP_BODY {
        return Err("response body too large".into());
    }
    let mut result = json!({
        "status": status,
        "headers": response_headers,
    });
    if bytes.len() <= 512 * 1024 {
        result["body"] = Value::String(String::from_utf8_lossy(&bytes).into_owned());
        result["bodySize"] = json!(bytes.len());
    } else {
        let descriptor = plugin_ipc::create(owner, bytes.len(), false)?;
        plugin_ipc::write(&descriptor.id, &bytes)?;
        result["bodyBuffer"] = serde_json::to_value(descriptor).map_err(|e| e.to_string())?;
        result["bodySize"] = json!(bytes.len());
    }
    Ok(result)
}

struct TcpEntry {
    owner: String,
    stream: TcpStream,
}

fn streams() -> &'static Mutex<HashMap<String, TcpEntry>> {
    static STREAMS: OnceLock<Mutex<HashMap<String, TcpEntry>>> = OnceLock::new();
    STREAMS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn tcp_open(
    owner: &str,
    targets: &[crate::plugin_host::TcpTarget],
    params: &Value,
) -> Result<Value, String> {
    let host = text_param(params, "host")?;
    let port = params
        .get("port")
        .and_then(Value::as_u64)
        .ok_or("missing port")? as u16;
    let normalized = plugin_permissions::normalize_domain(host)?;
    let allowed = targets.iter().any(|target| {
        target.port == port
            && plugin_permissions::normalize_domain(&target.host)
                .map(|value| value == normalized)
                .unwrap_or(false)
    });
    if !allowed {
        return Err("TCP target not granted".into());
    }
    let address = (normalized.as_str(), port)
        .to_socket_addrs()
        .map_err(|error| error.to_string())?
        .next()
        .ok_or("TCP target has no address")?;
    let stream = TcpStream::connect_timeout(&address, Duration::from_secs(30))
        .map_err(|error| error.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(120)))
        .map_err(|error| error.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(120)))
        .map_err(|error| error.to_string())?;
    let id = format!(
        "tcp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or_default()
    );
    streams()
        .lock()
        .map_err(|_| "TCP registry lock poisoned")?
        .insert(
            id.clone(),
            TcpEntry {
                owner: owner.to_owned(),
                stream,
            },
        );
    Ok(json!({"stream": id}))
}

pub fn tcp_read(owner: &str, params: &Value) -> Result<Value, String> {
    let id = text_param(params, "stream")?;
    let size = params.get("size").and_then(Value::as_u64).unwrap_or(65536) as usize;
    let size = size.clamp(1, MAX_TCP_CHUNK);
    let mut guard = streams().lock().map_err(|_| "TCP registry lock poisoned")?;
    let entry = guard.get_mut(id).ok_or("TCP stream not found")?;
    if entry.owner != owner {
        return Err("TCP stream belongs to another plugin".into());
    }
    let mut bytes = vec![0u8; size];
    let read = entry
        .stream
        .read(&mut bytes)
        .map_err(|error| error.to_string())?;
    bytes.truncate(read);
    drop(guard);
    if bytes.is_empty() {
        return Ok(json!({"buffer": Value::Null, "size": 0, "eof": true}));
    }
    let descriptor = plugin_ipc::create(owner, bytes.len(), false)?;
    plugin_ipc::write(&descriptor.id, &bytes)?;
    Ok(json!({"buffer": descriptor, "size": bytes.len(), "eof": false}))
}

pub fn tcp_write(owner: &str, params: &Value) -> Result<Value, String> {
    let id = text_param(params, "stream")?;
    let buffer = text_param(params, "buffer")?;
    let bytes = plugin_ipc::read(owner, buffer)?;
    let mut guard = streams().lock().map_err(|_| "TCP registry lock poisoned")?;
    let entry = guard.get_mut(id).ok_or("TCP stream not found")?;
    if entry.owner != owner {
        return Err("TCP stream belongs to another plugin".into());
    }
    entry
        .stream
        .write_all(&bytes)
        .map_err(|error| error.to_string())?;
    Ok(json!({"size": bytes.len()}))
}

pub fn tcp_close(owner: &str, params: &Value) -> Result<(), String> {
    let id = text_param(params, "stream")?;
    let mut guard = streams().lock().map_err(|_| "TCP registry lock poisoned")?;
    let entry = guard.get(id).ok_or("TCP stream not found")?;
    if entry.owner != owner {
        return Err("TCP stream belongs to another plugin".into());
    }
    guard.remove(id);
    Ok(())
}
