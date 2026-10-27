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
mod tcp;
use crate::plugins::security::policy::PermissionPolicy;
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

mod body;
mod request;
mod response;
mod upload;
use body::{BodyPipe, BodyReader};
pub use request::http_open;
pub use response::{http_body, http_close, http_read, http_response};
use upload::Upload;
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
#[cfg(test)]
mod tests;
