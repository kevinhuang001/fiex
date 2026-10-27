/// Push one chunk into a streaming request body. `eof` closes the body after
/// the chunk, so the final `eof` call may carry the last bytes.
use super::*;

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
