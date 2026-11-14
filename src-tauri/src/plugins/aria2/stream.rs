use super::{Shared, Stream, download, next_id, options, registry};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::{
    fs,
    io::Read,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub(crate) fn open(owner: &str, params: &Value) -> Result<Value, String> {
    let request = options::validate(params)?;
    let directory = tempfile::Builder::new()
        .prefix("fiex-aria2-")
        .tempdir()
        .map_err(|error| error.to_string())?;
    let options::PreparedDownload {
        url,
        options,
        size,
        segmented,
    } = options::prepare(request, directory.path());
    let payload = directory.path().join("payload");

    let shared = Arc::new(Mutex::new(Shared {
        done: false,
        error: None,
        total: size.unwrap_or(0),
    }));
    let (cancel, cancel_rx) = tokio::sync::watch::channel(false);
    let worker_shared = Arc::clone(&shared);
    std::thread::Builder::new()
        .name("fiex-aria2".into())
        .spawn({
            let worker_payload = payload.clone();
            move || download::run(url, options, worker_payload, size, worker_shared, cancel_rx)
        })
        .map_err(|error| error.to_string())?;

    let id = next_id();
    registry()
        .lock()
        .map_err(|_| "aria2 registry unavailable")?
        .insert(
            id.clone(),
            Arc::new(Mutex::new(Stream {
                owner: owner.to_string(),
                _directory: directory,
                payload,
                file: None,
                offset: 0,
                segmented,
                started: Instant::now(),
                shared,
                cancel,
            })),
        );
    Ok(json!({"streamId": id, "size": size}))
}

pub(crate) fn read(owner: &str, params: &Value) -> Result<Value, String> {
    let id = params
        .get("streamId")
        .and_then(Value::as_str)
        .ok_or("missing aria2 streamId")?;
    let size = params
        .get("size")
        .and_then(Value::as_u64)
        .unwrap_or(1024 * 1024)
        .clamp(1, 1024 * 1024) as usize;
    let stream = registry()
        .lock()
        .map_err(|_| "aria2 registry unavailable")?
        .get(id)
        .cloned()
        .ok_or("unknown aria2 stream")?;
    let mut stream = stream.lock().map_err(|_| "aria2 stream unavailable")?;
    if stream.owner != owner {
        return Err("unknown aria2 stream".into());
    }
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

        // With a single connection, the byte prefix is sequential; with
        // parallel ranges, the apparent file size can include unwritten holes.
        if done || !stream.segmented {
            let available = fs::metadata(&stream.payload)
                .map(|meta| meta.len())
                .unwrap_or(0);
            if available > stream.offset {
                if stream.file.is_none() {
                    stream.file = Some(fs::File::open(&stream.payload).map_err(|e| e.to_string())?);
                }
                let mut bytes = vec![0u8; size.min((available - stream.offset) as usize)];
                let count = stream
                    .file
                    .as_mut()
                    .ok_or("aria2 file unavailable")?
                    .read(&mut bytes)
                    .map_err(|e| e.to_string())?;
                if count > 0 {
                    bytes.truncate(count);
                    stream.offset += count as u64;
                    return Ok(json!({
                        "base64": STANDARD.encode(bytes),
                        "eof": done && stream.offset >= available
                    }));
                }
                if done {
                    return Err("aria2 output ended before its reported length".into());
                }
            }
            if done {
                return Ok(json!({"base64": "", "eof": true}));
            }
        }

        if stream.started.elapsed() > Duration::from_secs(6 * 60 * 60) {
            return Err("aria2 download timed out".into());
        }
        // Keep host calls bounded and let other operations cancel this stream.
        if wait.elapsed() > Duration::from_secs(1) {
            return Ok(json!({"base64": "", "eof": false}));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

pub(crate) fn close(owner: &str, params: &Value) -> Result<(), String> {
    let id = params
        .get("streamId")
        .and_then(Value::as_str)
        .ok_or("missing aria2 streamId")?;
    let stream = registry()
        .lock()
        .map_err(|_| "aria2 registry unavailable")?
        .get(id)
        .cloned();
    if let Some(stream) = stream {
        let stream = stream.lock().map_err(|_| "aria2 stream unavailable")?;
        if stream.owner != owner {
            return Err("unknown aria2 stream".into());
        }
        let _ = stream.cancel.send(true);
        drop(stream);
        registry()
            .lock()
            .map_err(|_| "aria2 registry unavailable")?
            .remove(id);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
