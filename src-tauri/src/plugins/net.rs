//! Host-mediated HTTP and TCP access for plugins.
//!
//! Plugins never open sockets. HTTP is a structured request/response API;
//! TCP is a controlled byte stream sharing HTTP's approved hostname/IP list.
//!
//! Besides the buffered body (`bodyBase64`), an authorized file handle
//! (`bodyReadHandle`) and the plugin-fed streaming body (`bodyStream` plus
//! `host.net.http.body`), every request uses the same authorization and
//! response registry. A streamed body is not buffered: `http_open` returns a
//! handle immediately while a worker thread pulls chunks from a bounded pipe.
#[path = "net/tcp.rs"]
mod tcp;
use crate::plugin_permission_policy::PermissionPolicy;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::{self, Read};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};
pub use tcp::{cleanup, tcp_close, tcp_open, tcp_read_bytes, tcp_shutdown, tcp_write_bytes};

/// Largest chunk a plugin may push into a streaming request body.
const MAX_BODY_CHUNK: usize = 4 * 1024 * 1024;
/// Largest streaming request body. Larger uploads need a resumable session.
const MAX_STREAM_BODY: u64 = 4 * 1024 * 1024 * 1024;
/// Bounded pipe depth between the host method and the reqwest reader.
const BODY_PIPE_CHUNKS: usize = 2;

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

fn http_headers(response: &reqwest::blocking::Response) -> BTreeMap<String, String> {
    response
        .headers()
        .iter()
        .filter_map(|(k, v)| {
            v.to_str()
                .ok()
                .map(|v| (k.as_str().to_owned(), v.to_owned()))
        })
        .collect()
}

fn http_id() -> Result<String, String> {
    Ok(format!(
        "http-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    ))
}

struct HttpEntry {
    owner: String,
    response: HttpEntryKind,
}

#[derive(Clone)]
enum HttpEntryKind {
    /// The response head is available and `host.net.http.read` streams it.
    Ready(Arc<Mutex<reqwest::blocking::Response>>),
    /// A streaming request is still uploading its body.
    Uploading(Arc<Upload>),
}

impl HttpEntry {
    fn abort(&self) {
        if let HttpEntryKind::Uploading(upload) = &self.response {
            upload.abort();
        }
    }
}

fn http_streams() -> &'static Mutex<HashMap<String, HttpEntry>> {
    static STREAMS: OnceLock<Mutex<HashMap<String, HttpEntry>>> = OnceLock::new();
    STREAMS.get_or_init(|| Mutex::new(HashMap::new()))
}

struct PipeState {
    chunks: VecDeque<Vec<u8>>,
    offset: usize,
    eof: bool,
    /// The worker no longer consumes bytes (request finished or failed).
    closed: bool,
    /// Cancelled by close/process teardown; carries the reason.
    abort: Option<String>,
    read_error: Option<String>,
    accepted: u64,
    delivered: u64,
}

/// Bounded producer/consumer pipe feeding `reqwest::blocking::Body::sized`.
/// The reader blocks (with a deadline) instead of buffering the body.
struct BodyPipe {
    state: Mutex<PipeState>,
    changed: Condvar,
    content_length: u64,
    deadline: Instant,
}

impl BodyPipe {
    fn new(content_length: u64, deadline: Instant) -> Self {
        Self {
            state: Mutex::new(PipeState {
                chunks: VecDeque::new(),
                offset: 0,
                eof: false,
                closed: false,
                abort: None,
                read_error: None,
                accepted: 0,
                delivered: 0,
            }),
            changed: Condvar::new(),
            content_length,
            deadline,
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, PipeState> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }

    /// Accept one chunk. `eof` closes the body after the chunk. The call waits
    /// only while the bounded pipe is full; a finished or failed request makes
    /// it fail instead of blocking forever.
    fn push(&self, bytes: Vec<u8>, eof: bool) -> Result<u64, String> {
        if bytes.len() > MAX_BODY_CHUNK {
            return Err("HTTP body chunk exceeds 4 MiB".into());
        }
        let mut state = self.state();
        loop {
            if let Some(reason) = &state.abort {
                return Err(reason.clone());
            }
            if state.eof {
                return Err("HTTP request body already ended".into());
            }
            let next_total = state
                .accepted
                .checked_add(bytes.len() as u64)
                .ok_or("HTTP request body size overflow")?;
            if next_total > self.content_length {
                return Err(format!(
                    "HTTP request body exceeds contentLength ({} > {})",
                    next_total, self.content_length
                ));
            }
            // The request may complete (and close the pipe) after exactly
            // contentLength bytes; a late final `eof` is still meaningful.
            if bytes.is_empty() && eof && next_total == self.content_length {
                state.eof = true;
                drop(state);
                self.changed.notify_all();
                return Ok(next_total);
            }
            if state.closed {
                return Err("HTTP request body is closed".into());
            }
            if bytes.is_empty() {
                if eof {
                    state.eof = true;
                }
                drop(state);
                self.changed.notify_all();
                return Ok(next_total);
            }
            if state.chunks.len() < BODY_PIPE_CHUNKS {
                state.accepted = next_total;
                state.chunks.push_back(bytes);
                if eof {
                    state.eof = true;
                }
                drop(state);
                self.changed.notify_all();
                return Ok(next_total);
            }
            let now = Instant::now();
            if now >= self.deadline {
                return Err("HTTP request body timed out before contentLength was reached".into());
            }
            let (next, _) = self
                .changed
                .wait_timeout(state, self.deadline - now)
                .unwrap_or_else(|error| error.into_inner());
            state = next;
        }
    }

    fn close(&self) {
        let mut state = self.state();
        state.closed = true;
        drop(state);
        self.changed.notify_all();
    }

    fn abort(&self, reason: &str) {
        let mut state = self.state();
        if state.abort.is_none() {
            state.abort = Some(reason.to_owned());
        }
        state.closed = true;
        drop(state);
        self.changed.notify_all();
    }

    fn read(&self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let mut state = self.state();
        loop {
            if let Some(reason) = &state.abort {
                return Err(io::Error::new(io::ErrorKind::BrokenPipe, reason.clone()));
            }
            if state.delivered >= self.content_length {
                return Ok(0);
            }
            if !state.chunks.is_empty() {
                let count = (state.chunks[0].len() - state.offset)
                    .min(output.len())
                    .min((self.content_length - state.delivered) as usize);
                {
                    let chunk = &state.chunks[0];
                    output[..count].copy_from_slice(&chunk[state.offset..state.offset + count]);
                }
                state.offset += count;
                state.delivered += count as u64;
                if state.offset == state.chunks[0].len() {
                    state.chunks.pop_front();
                    state.offset = 0;
                }
                drop(state);
                self.changed.notify_all();
                return Ok(count);
            }
            if state.eof {
                let message = format!(
                    "HTTP request body ended after {} of {} bytes",
                    state.delivered, self.content_length
                );
                state.read_error = Some(message.clone());
                return Err(io::Error::new(io::ErrorKind::InvalidData, message));
            }
            if state.closed {
                let message = "HTTP request body is closed".to_string();
                state.read_error = Some(message.clone());
                return Err(io::Error::new(io::ErrorKind::BrokenPipe, message));
            }
            let now = Instant::now();
            if now >= self.deadline {
                let message =
                    "HTTP request body timed out before contentLength was reached".to_string();
                state.read_error = Some(message.clone());
                return Err(io::Error::new(io::ErrorKind::TimedOut, message));
            }
            let (next, _) = self
                .changed
                .wait_timeout(state, self.deadline - now)
                .unwrap_or_else(|error| error.into_inner());
            state = next;
        }
    }

    fn read_error(&self) -> Option<String> {
        self.state().read_error.clone()
    }
}

struct BodyReader {
    pipe: Arc<BodyPipe>,
}

impl Read for BodyReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.pipe.read(output)
    }
}

enum UploadResult {
    Pending,
    Ready(Arc<Mutex<reqwest::blocking::Response>>),
    Failed(String),
}

/// One streamed upload. The worker publishes exactly one result; readers of
/// `host.net.http.response`/`read`/`close` observe it.
struct Upload {
    pipe: Arc<BodyPipe>,
    timeout: Duration,
    result: Mutex<UploadResult>,
    changed: Condvar,
}

impl Upload {
    fn new(content_length: u64, timeout: Duration) -> Self {
        Self {
            pipe: Arc::new(BodyPipe::new(content_length, Instant::now() + timeout)),
            timeout,
            result: Mutex::new(UploadResult::Pending),
            changed: Condvar::new(),
        }
    }

    fn publish(&self, result: Result<reqwest::blocking::Response, String>) {
        // Keep a single lock order (pipe, then result) with `abort`.
        self.pipe.close();
        {
            let mut state = self
                .result
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if matches!(*state, UploadResult::Pending) {
                *state = match result {
                    Ok(response) => UploadResult::Ready(Arc::new(Mutex::new(response))),
                    Err(error) => UploadResult::Failed(error),
                };
            }
        }
        self.changed.notify_all();
    }

    fn abort(&self) {
        self.pipe.abort("HTTP request cancelled");
        {
            let mut state = self
                .result
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if matches!(*state, UploadResult::Pending) {
                *state = UploadResult::Failed("HTTP request cancelled".into());
            }
        }
        self.changed.notify_all();
    }

    /// Wait for the worker to publish the response head. The deadline keeps a
    /// plugin from blocking forever if the worker cannot make progress.
    fn wait(&self) -> Result<Arc<Mutex<reqwest::blocking::Response>>, String> {
        let deadline = Instant::now() + self.timeout + Duration::from_secs(5);
        let mut state = self
            .result
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        loop {
            match &*state {
                UploadResult::Ready(response) => return Ok(response.clone()),
                UploadResult::Failed(error) => return Err(error.clone()),
                UploadResult::Pending => {}
            }
            let now = Instant::now();
            if now >= deadline {
                return Err("HTTP request timed out waiting for the response".into());
            }
            let (next, _) = self
                .changed
                .wait_timeout(state, deadline - now)
                .unwrap_or_else(|error| error.into_inner());
            state = next;
        }
    }
}

fn entry(owner: &str, id: &str) -> Result<HttpEntryKind, String> {
    http_streams()
        .lock()
        .map_err(|_| "HTTP registry unavailable")?
        .get(id)
        .filter(|entry| entry.owner == owner)
        .map(|entry| entry.response.clone())
        .ok_or_else(|| "HTTP response unavailable".to_string())
}

/// Validate the opt-in streaming parameters. Returns the declared body size.
/// A zero length is a valid empty streaming body (`Body::sized(reader, 0)`
/// never reads the pipe), so only the 4 GiB cap is enforced.
fn stream_length(params: &Value) -> Result<Option<u64>, String> {
    if !params
        .get("bodyStream")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(None);
    }
    if params.get("bodyReadHandle").is_some_and(|v| !v.is_null()) {
        return Err("ambiguous HTTP body: bodyStream cannot use bodyReadHandle".into());
    }
    if params.get("bodyBase64").is_some_and(|v| !v.is_null()) {
        return Err("ambiguous HTTP body: bodyStream cannot use bodyBase64".into());
    }
    let length = params
        .get("contentLength")
        .and_then(Value::as_u64)
        .ok_or("missing contentLength for streaming body")?;
    if length > MAX_STREAM_BODY {
        return Err("streaming HTTP body exceeds 4 GiB".into());
    }
    Ok(Some(length))
}

fn send_streamed(
    client: reqwest::blocking::Client,
    method: String,
    url: reqwest::Url,
    headers: reqwest::header::HeaderMap,
    reader: BodyReader,
    length: u64,
) -> Result<reqwest::blocking::Response, String> {
    let pipe = reader.pipe.clone();
    let request = client
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).map_err(|e| e.to_string())?,
            url,
        )
        .headers(headers)
        .body(reqwest::blocking::Body::sized(reader, length));
    let response = match request.send() {
        Ok(response) => response,
        // Prefer the pipe's own message ("timed out", "ended after ...") over
        // reqwest's generic "error reading request body".
        Err(error) => return Err(pipe.read_error().unwrap_or_else(|| error.to_string())),
    };
    let status = response.status().as_u16();
    // The body cannot be replayed for a redirect. A 308 without Location is
    // a resumable-upload acknowledgement, not a redirect.
    if matches!(status, 301 | 302 | 303 | 307 | 308)
        && (status != 308 || response.headers().contains_key(reqwest::header::LOCATION))
    {
        return Err(
            "streamed HTTP request cannot follow a redirect because the body is not replayable"
                .into(),
        );
    }
    Ok(response)
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
    let streaming_length = stream_length(params)?;
    let timeout = params
        .get("timeoutMs")
        .and_then(Value::as_u64)
        .unwrap_or(90000)
        .clamp(
            1,
            if streaming_length.is_some()
                || params
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
    if let Some(length) = streaming_length {
        if !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err("invalid HTTP URL".into());
        }
        if !policy.domain_allowed(url.host_str().ok_or("missing HTTP host")?)? {
            return Err("network target not granted".into());
        }
        let upload = Arc::new(Upload::new(length, Duration::from_millis(timeout)));
        let reader = BodyReader {
            pipe: upload.pipe.clone(),
        };
        let id = http_id()?;
        {
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
            entries.insert(
                id.clone(),
                HttpEntry {
                    owner: owner.into(),
                    response: HttpEntryKind::Uploading(upload.clone()),
                },
            );
        }
        let worker_upload = upload.clone();
        let spawned = std::thread::Builder::new()
            .name("fiex-http-upload".into())
            .spawn(move || {
                let result = send_streamed(client, method, url, headers, reader, length);
                worker_upload.publish(result);
            });
        if let Err(error) = spawned {
            if let Ok(mut entries) = http_streams().lock() {
                entries.remove(&id);
            }
            upload.abort();
            return Err(format!("cannot start HTTP request: {error}"));
        }
        // Return before the body is complete; the worker owns the request.
        return Ok(json!({"handle": id}));
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
        let headers = http_headers(&response);
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
        let id = http_id()?;
        entries.insert(
            id.clone(),
            HttpEntry {
                owner: owner.into(),
                response: HttpEntryKind::Ready(Arc::new(Mutex::new(response))),
            },
        );
        return Ok(json!({"stream": id, "status": status, "headers": headers}));
    }
    Err("HTTP redirect failed".into())
}

/// Push one chunk into a streaming request body. `eof` closes the body after
/// the chunk, so the final `eof` call may carry the last bytes.
pub fn http_body(owner: &str, params: &Value) -> Result<Value, String> {
    let id = text_param(params, "handle")?;
    let eof = params.get("eof").and_then(Value::as_bool).unwrap_or(false);
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(text_param(params, "base64")?)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_BODY_CHUNK {
        return Err("HTTP body chunk exceeds 4 MiB".into());
    }
    let upload = match entry(owner, id)? {
        HttpEntryKind::Uploading(upload) => upload,
        HttpEntryKind::Ready(_) => return Err("HTTP request body is closed".into()),
    };
    let written = upload.pipe.push(bytes, eof)?;
    Ok(json!({"written": written, "eof": eof}))
}

/// Wait for the response head of a streaming request. This is how a plugin
/// learns the status and headers after streaming the body.
pub fn http_response(owner: &str, params: &Value) -> Result<Value, String> {
    let id = text_param(params, "handle")?;
    let response = match entry(owner, id)? {
        HttpEntryKind::Ready(response) => response,
        HttpEntryKind::Uploading(upload) => upload.wait()?,
    };
    let response = response.lock().map_err(|_| "HTTP response unavailable")?;
    Ok(json!({
        "stream": id,
        "status": response.status().as_u16(),
        "headers": http_headers(&response),
    }))
}

pub fn http_read(owner: &str, params: &Value) -> Result<Value, String> {
    let id = text_param(params, "stream")?;
    let response = match entry(owner, id)? {
        HttpEntryKind::Ready(response) => response,
        // Reading a streaming handle first waits for the response head.
        HttpEntryKind::Uploading(upload) => upload.wait()?,
    };
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
    let entry = {
        let mut entries = http_streams()
            .lock()
            .map_err(|_| "HTTP registry unavailable")?;
        if entries.get(id).is_none_or(|entry| entry.owner != owner) {
            return Err("HTTP response unavailable".into());
        }
        entries.remove(id)
    };
    if let Some(entry) = entry {
        // Closing before the response arrives cancels the streamed upload.
        entry.abort();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    use std::collections::BTreeSet;
    use std::io::Write;
    use std::net::TcpListener;
    use std::thread;

    fn pipe_with(length: u64, timeout: Duration) -> Arc<BodyPipe> {
        Arc::new(BodyPipe::new(length, Instant::now() + timeout))
    }

    #[test]
    fn streaming_pipe_enforces_chunk_accounting_and_short_bodies() {
        let pipe = pipe_with(4, Duration::from_secs(5));
        assert_eq!(pipe.push(b"ab".to_vec(), false).unwrap(), 2);
        // The declared contentLength is the hard upper bound.
        assert!(pipe.push(b"cde".to_vec(), false).is_err());
        assert_eq!(pipe.push(b"cd".to_vec(), true).unwrap(), 4);
        // Writes after eof are rejected.
        assert!(pipe.push(Vec::new(), true).is_err());
        assert!(pipe.push(b"x".to_vec(), false).is_err());

        let mut reader = BodyReader { pipe: pipe.clone() };
        let mut output = [0u8; 2];
        assert_eq!(reader.read(&mut output).unwrap(), 2);
        assert_eq!(&output, b"ab");
        let mut rest = [0u8; 8];
        assert_eq!(reader.read(&mut rest).unwrap(), 2);
        assert_eq!(&rest[..2], b"cd");
        // Exactly contentLength bytes: the reader stops without waiting for
        // another chunk.
        assert_eq!(reader.read(&mut rest).unwrap(), 0);

        // A short eof fails the body instead of hanging.
        let pipe = pipe_with(3, Duration::from_secs(5));
        pipe.push(b"ab".to_vec(), true).unwrap();
        let mut reader = BodyReader { pipe };
        let mut output = [0u8; 8];
        assert_eq!(reader.read(&mut output).unwrap(), 2);
        let error = reader.read(&mut output).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("2 of 3 bytes"));
    }

    #[test]
    fn streaming_pipe_blocks_for_chunks_and_times_out_without_them() {
        let pipe = pipe_with(3, Duration::from_secs(5));
        let reader_pipe = pipe.clone();
        let task = thread::spawn(move || {
            let mut reader = BodyReader { pipe: reader_pipe };
            let mut output = [0u8; 8];
            reader.read(&mut output).map(|read| output[..read].to_vec())
        });
        thread::sleep(Duration::from_millis(50));
        pipe.push(b"xyz".to_vec(), true).unwrap();
        assert_eq!(task.join().unwrap().unwrap(), b"xyz");

        // A channel that never reaches contentLength fails with a deadline
        // error, and the failure is visible to the worker.
        let pipe = pipe_with(1, Duration::from_millis(80));
        let mut reader = BodyReader { pipe: pipe.clone() };
        let error = reader.read(&mut [0u8; 4]).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(pipe
            .read_error()
            .is_some_and(|message| message.contains("timed out")));
    }

    #[test]
    fn streaming_parameters_are_validated_before_any_io() {
        let declared = BTreeSet::new();
        let policy = PermissionPolicy::new("fixture", &declared);
        let open = |params: Value| http_open("owner", &policy, &params);

        let error = open(json!({"url": "https://example.com", "bodyStream": true})).unwrap_err();
        assert!(error.contains("contentLength"));
        let error = open(json!({
            "url": "https://example.com",
            "bodyStream": true,
            "contentLength": 1,
            "bodyBase64": ""
        }))
        .unwrap_err();
        assert!(error.contains("bodyBase64"));
        let error = open(json!({
            "url": "https://example.com",
            "bodyStream": true,
            "contentLength": 1,
            "bodyReadHandle": "handle"
        }))
        .unwrap_err();
        assert!(error.contains("bodyReadHandle"));
        let error = open(json!({
            "url": "https://example.com",
            "bodyStream": true,
            "contentLength": MAX_STREAM_BODY + 1
        }))
        .unwrap_err();
        assert!(error.contains("4 GiB"));
    }

    fn test_client() -> reqwest::blocking::Client {
        reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .build()
            .unwrap()
    }

    fn serve_once(response: &'static [u8]) -> (reqwest::Url, thread::JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = reqwest::Url::parse(&format!("http://{}/upload", listener.local_addr().unwrap()))
            .unwrap();
        let task = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut chunk = [0u8; 1024];
            let mut head_end = 0;
            while head_end == 0 {
                let read = socket.read(&mut chunk).unwrap();
                if read == 0 {
                    return request;
                }
                request.extend_from_slice(&chunk[..read]);
                head_end = request
                    .windows(4)
                    .position(|window| window == b"\r\n\r\n")
                    .map(|position| position + 4)
                    .unwrap_or(0);
            }
            let head = String::from_utf8_lossy(&request[..head_end]).to_ascii_lowercase();
            let length: usize = head
                .lines()
                .find_map(|line| line.strip_prefix("content-length:"))
                .and_then(|value| value.trim().parse().ok())
                .unwrap_or(0);
            while request.len() < head_end + length {
                let read = socket.read(&mut chunk).unwrap();
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..read]);
            }
            socket.write_all(response).unwrap();
            request
        });
        (url, task)
    }

    fn send_to(url: reqwest::Url, length: u64) -> Result<reqwest::blocking::Response, String> {
        let pipe = pipe_with(length, Duration::from_secs(5));
        if length > 0 {
            pipe.push(vec![7; length as usize], true).unwrap();
        }
        send_streamed(
            test_client(),
            "PUT".into(),
            url,
            reqwest::header::HeaderMap::new(),
            BodyReader { pipe },
            length,
        )
    }

    #[test]
    fn streamed_requests_do_not_follow_redirects() {
        let (url, server) = serve_once(
            b"HTTP/1.1 307 Temporary Redirect\r\nLocation: /again\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        );
        let error = send_to(url, 4).unwrap_err();
        assert!(error.contains("redirect"), "unexpected error: {error}");
        let request = server.join().unwrap();
        assert!(request.starts_with(b"PUT /upload"));
    }

    #[test]
    fn streamed_308_without_location_is_a_normal_response() {
        let (url, server) = serve_once(
            b"HTTP/1.1 308 Permanent Redirect\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        );
        let response = send_to(url, 4).unwrap();
        assert_eq!(response.status().as_u16(), 308);
        drop(response);
        server.join().unwrap();
    }

    #[test]
    fn streaming_body_chunks_accept_the_final_eof_and_reject_late_writes() {
        // The SDK sends `eof` even when the worker already consumed exactly
        // contentLength; that final call must be a no-op, not an error.
        let upload = Upload::new(2, Duration::from_secs(5));
        assert_eq!(upload.pipe.push(b"hi".to_vec(), false).unwrap(), 2);
        upload.pipe.close();
        assert_eq!(upload.pipe.push(Vec::new(), true).unwrap(), 2);
        assert!(upload.pipe.push(b"x".to_vec(), false).is_err());
    }

    #[test]
    fn body_method_enforces_owner_and_chunk_limits() {
        let upload = Arc::new(Upload::new(8, Duration::from_secs(5)));
        let id = http_id().unwrap();
        http_streams().lock().unwrap().insert(
            id.clone(),
            HttpEntry {
                owner: "owner".into(),
                response: HttpEntryKind::Uploading(upload),
            },
        );
        let chunk = STANDARD.encode(vec![1u8; MAX_BODY_CHUNK + 1]);
        let error = http_body(
            "owner",
            &json!({"handle": id, "base64": chunk, "eof": false}),
        )
        .unwrap_err();
        assert!(error.contains("4 MiB"));
        let error =
            http_body("other", &json!({"handle": id, "base64": "", "eof": true})).unwrap_err();
        assert!(error.contains("unavailable"));
        http_close("owner", &json!({"stream": id})).unwrap();
    }
}
