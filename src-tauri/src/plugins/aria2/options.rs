use aria2_core::request::request_group::DownloadOptions;
use serde_json::Value;
use std::path::Path;

const MIB: u64 = 1024 * 1024;

pub(super) struct DownloadRequest {
    url: String,
    headers: Vec<String>,
    size: Option<u64>,
    connection_cap: u16,
    split_cap: u16,
    min_split: u64,
    timeout_ms: u64,
    allow_invalid_certs: bool,
}

pub(super) struct PreparedDownload {
    pub(super) url: String,
    pub(super) options: DownloadOptions,
    pub(super) size: Option<u64>,
    pub(super) segmented: bool,
}

pub(super) fn validate(params: &Value) -> Result<DownloadRequest, String> {
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
    let size = params.get("size").and_then(Value::as_u64);
    let connection_cap = number(params, "connections", 16, 1, 16) as u16;
    let split_cap = number(params, "split", 16, 1, 16) as u16;
    let min_split = number(params, "minSplitSize", MIB, MIB, 1024 * MIB);
    let timeout_ms = number(params, "timeoutMs", 300_000, 1_000, 86_400_000);
    let allow_invalid_certs = params
        .get("allowInvalidCerts")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Ok(DownloadRequest {
        url: url.to_string(),
        headers,
        size,
        connection_cap,
        split_cap,
        min_split,
        timeout_ms,
        allow_invalid_certs,
    })
}

pub(super) fn prepare(request: DownloadRequest, directory: &Path) -> PreparedDownload {
    let DownloadRequest {
        url,
        headers,
        size,
        connection_cap,
        split_cap,
        min_split,
        timeout_ms,
        allow_invalid_certs,
    } = request;
    let (connections, split) = download_parallelism(size, connection_cap, split_cap, min_split);

    let (all_proxy, proxy_user, proxy_pass) = proxy();
    let options = DownloadOptions {
        dir: Some(directory.to_string_lossy().into_owned()),
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

    PreparedDownload {
        url,
        options,
        size,
        segmented: split > 1,
    }
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

/// Select range count centrally. Caller options are caps, not a requirement
/// to open that many connections for every file. Unknown sizes start
/// sequentially so VFS previews can read a growing contiguous prefix.
fn download_parallelism(
    size: Option<u64>,
    connection_cap: u16,
    split_cap: u16,
    min_split: u64,
) -> (u16, u16) {
    let desired = match size {
        Some(bytes) if bytes >= 256 * MIB => 16,
        Some(bytes) if bytes >= 64 * MIB => 8,
        Some(bytes) if bytes >= 16 * MIB => 4,
        Some(bytes) if bytes >= 4 * MIB => 2,
        _ => 1,
    };
    let by_size = size
        .map(|bytes| (bytes / min_split.max(1)).clamp(1, u16::MAX as u64) as u16)
        .unwrap_or(1);
    let parallel = desired
        .min(connection_cap)
        .min(split_cap)
        .min(by_size)
        .max(1);
    (parallel, parallel)
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

#[cfg(test)]
mod tests;
