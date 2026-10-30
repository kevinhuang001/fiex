//! Host-owned aria2 streams used by the SDK `ctx.aria2()` API.
//!
//! The engine is the pure-Rust aria2 rewrite (`aria2-core`), so no external
//! `aria2c` binary is required. Each download owns a small runtime thread; the
//! host API only exposes a growing file as a stream, exactly like the old
//! process-backed implementation.
use aria2_core::{
    request::request_group::DownloadOptions, request::request_group_man::RequestGroupMan,
    DownloadEngine, DownloadStatus,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs,
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};

struct Shared {
    done: bool,
    error: Option<String>,
    total: u64,
}

struct Stream {
    _directory: tempfile::TempDir,
    payload: PathBuf,
    offset: u64,
    started: Instant,
    shared: Arc<Mutex<Shared>>,
    cancel: tokio::sync::watch::Sender<bool>,
}

fn registry() -> &'static Mutex<HashMap<String, Arc<Mutex<Stream>>>> {
    static STREAMS: OnceLock<Mutex<HashMap<String, Arc<Mutex<Stream>>>>> = OnceLock::new();
    STREAMS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn next_id() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    format!("aria2-{}", NEXT.fetch_add(1, Ordering::Relaxed))
}

fn header(value: &Value) -> Result<Vec<String>, String> {
    let Some(object) = value.as_object() else {
        return Ok(Default::default());
    };
    if object.len() > 16 {
        return Err("too many aria2 headers".into());
    }
    let mut headers = Vec::with_capacity(object.len());
    for (name, value) in object {
        let value = value.as_str().ok_or("invalid aria2 header")?;
        if name.is_empty()
            || name.len() > 256
            || value.len() > 4096
            || name.chars().any(|c| c == '\r' || c == '\n' || c == '\0')
            || value.chars().any(|c| c == '\r' || c == '\n' || c == '\0')
        {
            return Err("invalid aria2 header".into());
        }
        headers.push(format!("{name}: {value}"));
    }
    Ok(headers)
}

fn number(params: &Value, key: &str, default: u64, min: u64, max: u64) -> u64 {
    params
        .get(key)
        .and_then(Value::as_u64)
        .unwrap_or(default)
        .clamp(min, max)
}

fn proxy() -> (Option<String>, Option<String>, Option<String>) {
    let Some(raw) = crate::platform::system_proxy::proxy_url() else {
        return (None, None, None);
    };
    let Ok(mut url) = reqwest::Url::parse(&raw) else {
        return (Some(raw), None, None);
    };
    let user = (!url.username().is_empty()).then(|| url.username().to_string());
    let pass = url.password().map(str::to_string);
    let _ = url.set_username("");
    let _ = url.set_password(None);
    (Some(url.to_string()), user, pass)
}

fn run(
    url: String,
    options: DownloadOptions,
    shared: Arc<Mutex<Shared>>,
    mut cancel: tokio::sync::watch::Receiver<bool>,
) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build();
    let result = match runtime {
        Ok(runtime) => runtime.block_on(async move {
            let groups = Arc::new(RequestGroupMan::new());
            let engine = match DownloadEngine::new().start_with_request_group_man(groups) {
                Ok(engine) => engine,
                Err(error) => return Err(error.to_string()),
            };
            let download = match engine.downloads().add_uri(vec![url], options) {
                Ok(download) => download,
                Err(error) => {
                    let _ = engine.shutdown_and_wait().await;
                    return Err(error.to_string());
                }
            };
            let outcome = tokio::select! {
                result = download.wait() => result.map_err(|error| error.to_string()),
                _ = async {
                    while cancel.changed().await.is_ok() {
                        if *cancel.borrow() {
                            break;
                        }
                    }
                } => {
                    let _ = download.remove();
                    Err("aria2 download cancelled".to_string())
                }
            };
            let _ = engine.shutdown_and_wait().await;
            outcome
        }),
        Err(error) => Err(error.to_string()),
    };
    if let Ok(mut shared) = shared.lock() {
        match result {
            Ok(result) if result.status == DownloadStatus::Complete => {
                shared.done = true;
                if result.total_length > 0 {
                    shared.total = result.total_length;
                }
            }
            Ok(result) => {
                shared.done = true;
                shared.error = Some(if result.message.is_empty() {
                    format!("aria2 download ended with {:?}", result.status)
                } else {
                    result.message
                });
            }
            Err(error) => {
                shared.done = true;
                shared.error = Some(error);
            }
        }
    }
}

pub(crate) fn open(params: &Value) -> Result<Value, String> {
    let url = params
        .get("url")
        .and_then(Value::as_str)
        .ok_or("missing aria2 url")?;
    if !(url.starts_with("https://") || url.starts_with("http://"))
        || url.len() > 16_384
        || url
            .chars()
            .any(|c| c.is_control() || c == '\r' || c == '\n' || c == ' ')
    {
        return Err("invalid aria2 url".into());
    }
    let headers = header(params.get("headers").unwrap_or(&Value::Null))?;
    let connections = number(params, "connections", 8, 1, 16) as u16;
    let split = number(params, "split", 8, 1, 16) as u16;
    let min_split = number(
        params,
        "minSplitSize",
        1024 * 1024,
        1024 * 1024,
        1024 * 1024 * 1024,
    );
    let timeout_ms = number(params, "timeoutMs", 300_000, 1_000, 86_400_000);
    let allow_invalid_certs = params
        .get("allowInvalidCerts")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let size = params.get("size").and_then(Value::as_u64).unwrap_or(0);
    let directory = tempfile::Builder::new()
        .prefix("fiex-aria2-")
        .tempdir()
        .map_err(|error| error.to_string())?;
    let payload = directory.path().join("payload");

    let (all_proxy, proxy_user, proxy_pass) = proxy();
    let options = DownloadOptions {
        dir: Some(directory.path().to_string_lossy().into_owned()),
        out: Some("payload".into()),
        split: Some(split),
        max_connection_per_server: Some(connections),
        min_split_size: Some(min_split),
        allow_overwrite: true,
        auto_file_renaming: false,
        file_allocation: Some("none".into()),
        timeout: Some((timeout_ms / 1000).max(1)),
        connect_timeout: Some(30),
        check_certificate: !allow_invalid_certs,
        header: headers,
        all_proxy,
        all_proxy_user: proxy_user,
        all_proxy_passwd: proxy_pass,
        no_proxy: std::env::var("NO_PROXY")
            .or_else(|_| std::env::var("no_proxy"))
            .ok(),
        ..Default::default()
    };

    let shared = Arc::new(Mutex::new(Shared {
        done: false,
        error: None,
        total: size,
    }));
    let (cancel, cancel_rx) = tokio::sync::watch::channel(false);
    let worker_shared = Arc::clone(&shared);
    let worker_cancel = cancel_rx;
    let worker_url = url.to_string();
    std::thread::Builder::new()
        .name("fiex-aria2".into())
        .spawn(move || run(worker_url, options, worker_shared, worker_cancel))
        .map_err(|error| error.to_string())?;

    let id = next_id();
    registry()
        .lock()
        .map_err(|_| "aria2 registry unavailable")?
        .insert(
            id.clone(),
            Arc::new(Mutex::new(Stream {
                _directory: directory,
                payload,
                offset: 0,
                started: Instant::now(),
                shared,
                cancel,
            })),
        );
    Ok(json!({"streamId": id, "size": size}))
}

pub(crate) fn read(params: &Value) -> Result<Value, String> {
    let id = params
        .get("streamId")
        .and_then(Value::as_str)
        .ok_or("missing aria2 streamId")?;
    let size = params
        .get("size")
        .and_then(Value::as_u64)
        .unwrap_or(256 * 1024)
        .clamp(1, 256 * 1024) as usize;
    let stream = registry()
        .lock()
        .map_err(|_| "aria2 registry unavailable")?
        .get(id)
        .cloned()
        .ok_or("unknown aria2 stream")?;
    let mut stream = stream.lock().map_err(|_| "aria2 stream unavailable")?;
    let wait = Instant::now();
    loop {
        let (done, error) = {
            let shared = stream
                .shared
                .lock()
                .map_err(|_| "aria2 state unavailable")?;
            (shared.done, shared.error.clone())
        };
        if let Some(error) = error {
            return Err(error);
        }
        let available = fs::metadata(&stream.payload)
            .map(|meta| meta.len())
            .unwrap_or(0);
        if available > stream.offset {
            let mut file = fs::File::open(&stream.payload).map_err(|error| error.to_string())?;
            file.seek(SeekFrom::Start(stream.offset))
                .map_err(|error| error.to_string())?;
            let mut bytes = vec![0u8; size];
            let read = file.read(&mut bytes).map_err(|error| error.to_string())?;
            bytes.truncate(read);
            stream.offset += read as u64;
            let eof = done && stream.offset >= available;
            return Ok(json!({
                "base64": STANDARD.encode(bytes),
                "eof": eof,
            }));
        }
        if done {
            return Ok(json!({"base64": "", "eof": true}));
        }
        if stream.started.elapsed() > Duration::from_secs(6 * 60 * 60) {
            return Err("aria2 download timed out".into());
        }
        if wait.elapsed() > Duration::from_secs(5) {
            return Ok(json!({"base64": "", "eof": false}));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

pub(crate) fn close(params: &Value) -> Result<(), String> {
    let id = params
        .get("streamId")
        .and_then(Value::as_str)
        .ok_or("missing aria2 streamId")?;
    if let Some(stream) = registry()
        .lock()
        .map_err(|_| "aria2 registry unavailable")?
        .remove(id)
    {
        if let Ok(stream) = stream.lock() {
            let _ = stream.cancel.send(true);
        }
    }
    Ok(())
}
