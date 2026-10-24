//! Host-mediated HTTP and TCP access for plugins.
//!
//! Plugins never open sockets. HTTP is a structured request/response API;
//! TCP is a controlled byte stream sharing HTTP's approved hostname/IP list.
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
    permission_id: &str,
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
    if !plugin_permissions::domain_allowed(permission_id, declared, host)? {
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
    let allow_invalid_certs = params
        .get("allowInvalidCerts")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .redirect(reqwest::redirect::Policy::none())
        .danger_accept_invalid_certs(allow_invalid_certs)
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
    let mut bytes = Vec::new();
    response
        .take((MAX_HTTP_BODY + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > MAX_HTTP_BODY {
        return Err("response body too large".into());
    }
    let mut result = json!({
        "status": status,
        "headers": response_headers,
    });
    if bytes.len() <= 512 * 1024 {
        use base64::Engine;
        result["bodyBase64"] = json!(base64::engine::general_purpose::STANDARD.encode(&bytes));
        if let Ok(text) = std::str::from_utf8(&bytes) {
            result["body"] = json!(text);
        }
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
    stream: std::sync::Arc<Mutex<TcpStream>>,
}

fn streams() -> &'static Mutex<HashMap<String, TcpEntry>> {
    static STREAMS: OnceLock<Mutex<HashMap<String, TcpEntry>>> = OnceLock::new();
    STREAMS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn tcp_open(
    owner: &str,
    permission_id: &str,
    declared: &std::collections::BTreeSet<String>,
    params: &Value,
) -> Result<Value, String> {
    let host = text_param(params, "host")?;
    let port = params
        .get("port")
        .and_then(Value::as_u64)
        .filter(|p| (1..=65535).contains(p))
        .ok_or("invalid TCP port")? as u16;
    let normalized = plugin_permissions::normalize_domain(host)?;
    let allowed = plugin_permissions::domain_allowed(permission_id, declared, &normalized)?;
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
    let mut entries = streams().lock().map_err(|_| "TCP registry unavailable")?;
    if entries
        .values()
        .filter(|entry| entry.owner == owner)
        .count()
        >= 32
    {
        return Err("too many TCP connections".into());
    }
    entries.insert(
        id.clone(),
        TcpEntry {
            owner: owner.into(),
            stream: std::sync::Arc::new(Mutex::new(stream)),
        },
    );
    Ok(json!({"stream": id, "peerAddress": address.to_string()}))
}

fn tcp_stream(owner: &str, id: &str) -> Result<std::sync::Arc<Mutex<TcpStream>>, String> {
    let entries = streams().lock().map_err(|_| "TCP registry unavailable")?;
    entries
        .get(id)
        .filter(|entry| entry.owner == owner)
        .map(|entry| entry.stream.clone())
        .ok_or_else(|| "TCP stream unavailable".into())
}
pub fn tcp_read(owner: &str, params: &Value) -> Result<Value, String> {
    let size = params
        .get("size")
        .and_then(Value::as_u64)
        .unwrap_or(65536)
        .clamp(1, MAX_TCP_CHUNK as u64) as usize;
    let stream = tcp_stream(owner, text_param(params, "stream")?)?;
    let mut bytes = vec![0; size];
    let read = stream
        .lock()
        .map_err(|_| "TCP connection unavailable")?
        .read(&mut bytes)
        .map_err(|e| e.to_string())?;
    bytes.truncate(read);
    if bytes.is_empty() {
        return Ok(json!({"buffer": Value::Null, "size": 0, "eof": true}));
    }
    let descriptor = plugin_ipc::create(owner, bytes.len(), false)?;
    plugin_ipc::write(&descriptor.id, &bytes)?;
    Ok(json!({"buffer": descriptor, "size": read, "eof": false}))
}
pub fn tcp_write(owner: &str, params: &Value) -> Result<Value, String> {
    let bytes = plugin_ipc::read(owner, text_param(params, "buffer")?)?;
    let stream = tcp_stream(owner, text_param(params, "stream")?)?;
    stream
        .lock()
        .map_err(|_| "TCP connection unavailable")?
        .write_all(&bytes)
        .map_err(|e| e.to_string())?;
    Ok(json!({"size":bytes.len()}))
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

pub fn cleanup(owner: &str) {
    if let Ok(mut entries) = http_streams().lock() {
        entries.retain(|_, entry| entry.owner != owner);
    }
    if let Ok(mut entries) = streams().lock() {
        entries.retain(|_, entry| entry.owner != owner);
    }
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
    permission_id: &str,
    declared: &std::collections::BTreeSet<String>,
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
        if !plugin_permissions::domain_allowed(
            permission_id,
            declared,
            url.host_str().ok_or("missing HTTP host")?,
        )? {
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

pub fn tcp_read_bytes(owner: &str, params: &Value) -> Result<Value, String> {
    let size = params
        .get("size")
        .and_then(Value::as_u64)
        .unwrap_or(65536)
        .clamp(1, 262144) as usize;
    let stream = tcp_stream(owner, text_param(params, "stream")?)?;
    let mut bytes = vec![0; size];
    let read = stream
        .lock()
        .map_err(|_| "TCP connection unavailable")?
        .read(&mut bytes)
        .map_err(|e| e.to_string())?;
    bytes.truncate(read);
    use base64::Engine;
    Ok(
        json!({"base64":base64::engine::general_purpose::STANDARD.encode(bytes),"size":read,"eof":read==0}),
    )
}
pub fn tcp_write_bytes(owner: &str, params: &Value) -> Result<Value, String> {
    let encoded = text_param(params, "base64")?;
    if encoded.len() > 349528 {
        return Err("TCP chunk too large".into());
    }
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| e.to_string())?;
    let stream = tcp_stream(owner, text_param(params, "stream")?)?;
    stream
        .lock()
        .map_err(|_| "TCP connection unavailable")?
        .write_all(&bytes)
        .map_err(|e| e.to_string())?;
    Ok(json!({"size":bytes.len()}))
}
