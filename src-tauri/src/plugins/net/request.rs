use super::*;

pub(super) fn stream_length(params: &Value) -> Result<Option<u64>, String> {
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

pub(super) fn send_streamed(
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
        .map(|id| crate::plugins::resource::fs_access::http_body(owner, id))
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
