//! Host-owned aria2 streams used by the SDK `ctx.aria2()` API.
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};

struct Stream {
    process: Child,
    _directory: tempfile::TempDir,
    payload: PathBuf,
    offset: u64,
    status: Option<bool>,
    started: Instant,
}

impl Drop for Stream {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

fn registry() -> &'static Mutex<HashMap<String, Arc<Mutex<Stream>>>> {
    static STREAMS: OnceLock<Mutex<HashMap<String, Arc<Mutex<Stream>>>>> = OnceLock::new();
    STREAMS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn next_id() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    format!("aria2-{}", NEXT.fetch_add(1, Ordering::Relaxed))
}

fn header(value: &Value) -> Result<std::collections::BTreeMap<String, String>, String> {
    let Some(object) = value.as_object() else {
        return Ok(Default::default());
    };
    if object.len() > 16 {
        return Err("too many aria2 headers".into());
    }
    let mut headers = std::collections::BTreeMap::new();
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
        headers.insert(name.clone(), value.to_string());
    }
    Ok(headers)
}

fn number(params: &Value, key: &str, default: u64, min: u64, max: u64) -> Result<u64, String> {
    Ok(params
        .get(key)
        .and_then(Value::as_u64)
        .unwrap_or(default)
        .clamp(min, max))
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
    let connections = number(params, "connections", 8, 1, 16)?;
    let split = number(params, "split", 8, 1, 16)?;
    let min_split = number(
        params,
        "minSplitSize",
        1024 * 1024,
        1024 * 1024,
        1024 * 1024 * 1024,
    )?;
    let timeout_ms = number(params, "timeoutMs", 300_000, 1_000, 86_400_000)?;
    let allow_invalid_certs = params
        .get("allowInvalidCerts")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let size = params.get("size").and_then(Value::as_u64);

    let directory = tempfile::Builder::new()
        .prefix("fiex-aria2-")
        .tempdir()
        .map_err(|error| error.to_string())?;
    let input = directory.path().join("input");
    let payload = directory.path().join("payload");
    {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&input)
            .map_err(|error| error.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&input, fs::Permissions::from_mode(0o600));
        }
        writeln!(file, "{url}").map_err(|error| error.to_string())?;
        for (name, value) in headers {
            writeln!(file, "  header={name}: {value}").map_err(|error| error.to_string())?;
        }
    }
    let mut command = Command::new("aria2c");
    command
        .arg("--no-conf")
        .arg("--quiet=true")
        .arg("--summary-interval=0")
        .arg("--console-log-level=error")
        .arg("--file-allocation=none")
        .arg("--auto-file-renaming=false")
        .arg("--allow-overwrite=true")
        .arg("--max-tries=3")
        .arg("--retry-wait=1")
        .arg(format!("--split={split}"))
        .arg(format!("--max-connection-per-server={connections}"))
        .arg(format!("--min-split-size={min_split}"))
        .arg(format!("--timeout={}", (timeout_ms / 1000).max(1)))
        .arg(format!("--input-file={}", input.display()))
        .arg(format!("--dir={}", directory.path().display()))
        .arg("--out=payload")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if allow_invalid_certs {
        command.arg("--check-certificate=false");
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let process = command.spawn().map_err(|error| error.to_string())?;
    let id = next_id();
    registry()
        .lock()
        .map_err(|_| "aria2 registry unavailable")?
        .insert(
            id.clone(),
            Arc::new(Mutex::new(Stream {
                process,
                _directory: directory,
                payload,
                offset: 0,
                status: None,
                started: Instant::now(),
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
    loop {
        if stream.status.is_none() {
            if let Ok(Some(status)) = stream.process.try_wait() {
                stream.status = Some(status.success());
            }
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
            let eof = stream.status.is_some() && stream.offset >= available;
            return Ok(json!({
                "base64": STANDARD.encode(bytes),
                "eof": eof,
            }));
        }
        match stream.status {
            Some(true) => return Ok(json!({"base64": "", "eof": true})),
            Some(false) => return Err("aria2 download failed".into()),
            None => {}
        }
        if stream.started.elapsed() > Duration::from_secs(6 * 60 * 60) {
            return Err("aria2 download timed out".into());
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
        if let Ok(mut stream) = stream.lock() {
            let _ = stream.process.kill();
            let _ = stream.process.wait();
        }
    }
    Ok(())
}
