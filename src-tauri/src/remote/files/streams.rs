use super::*;

pub(crate) fn fetch_remote(
    conn: &StoredConnection,
    rel: &str,
    dest: &Path,
    options: &TransferOptions,
) -> Result<u64, String> {
    use base64::Engine;
    use std::io::Write;

    const CHUNK: u64 = 256 * 1024;
    const MAX_TRANSFER: u64 = i64::MAX as u64;
    let plugin_id = ensure_provider_access(conn)?;
    if options.cancelled() {
        return Err(crate::core::operations::CANCELLED.into());
    }
    super::super::config::check_cache_path(conn, dest)?;
    let parent = dest.parent().ok_or("invalid download destination")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut staging = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    let opened = crate::plugins::host::global().invoke(
        &plugin_id,
        "fs.openRead",
        serde_json::json!({
            "mountId": conn.mount_id,
            "connection": provider_config(conn),
            "nodeId": rel
        }),
    )?;
    let stream_id = opened
        .get("streamId")
        .and_then(serde_json::Value::as_str)
        .ok_or("provider returned no read stream")?
        .to_string();
    let total = opened
        .get("size")
        .and_then(serde_json::Value::as_u64)
        .filter(|size| *size <= MAX_TRANSFER);
    let mut offset = 0u64;
    struct ReadLease<'a> {
        plugin: &'a str,
        mount_id: &'a str,
        stream: &'a str,
    }
    impl Drop for ReadLease<'_> {
        fn drop(&mut self) {
            let _ = crate::plugins::host::global().invoke(
                self.plugin,
                "fs.closeRead",
                serde_json::json!({"mountId": self.mount_id, "streamId": self.stream}),
            );
        }
    }
    let _lease = ReadLease {
        plugin: &plugin_id,
        mount_id: &conn.mount_id,
        stream: &stream_id,
    };
    loop {
        if options.cancelled() {
            return Err(crate::core::operations::CANCELLED.into());
        }
        if offset >= MAX_TRANSFER {
            return Err("remote file exceeds maximum allowed transfer size".into());
        }
        let response = match crate::plugins::host::global().invoke(
            &plugin_id,
            "fs.readStream",
            serde_json::json!({
                "mountId": conn.mount_id,
                "streamId": &stream_id,
                "offset": offset,
                "maxBytes": CHUNK
            }),
        ) {
            Ok(response) => response,
            Err(error) => {
                return Err(error);
            }
        };
        let encoded = response
            .get("base64")
            .and_then(serde_json::Value::as_str)
            .ok_or("provider returned no byte data")?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|_| "provider returned invalid base64")?;
        if bytes.len() > CHUNK as usize || offset + bytes.len() as u64 > MAX_TRANSFER {
            return Err("provider exceeded the transfer frame size".into());
        }
        let eof = response
            .get("eof")
            .and_then(serde_json::Value::as_bool)
            .ok_or("provider returned no EOF flag")?;
        if bytes.is_empty() && !eof {
            return Err("provider returned an empty nonfinal chunk".into());
        }
        staging.write_all(&bytes).map_err(|e| e.to_string())?;
        offset += bytes.len() as u64;
        options.report(offset, total);
        if eof {
            break;
        }
    }
    if total.is_some_and(|size| offset != size) {
        return Err("remote stream length does not match advertised size".into());
    }
    staging.as_file().sync_all().map_err(|e| e.to_string())?;
    staging.persist(dest).map_err(|e| e.error.to_string())?;
    Ok(offset)
}
/// Native provider-backed listing. Protocol I/O is owned by the installed,
/// explicitly approved plugin; Core only resolves an existing connection and
/// forwards a page request. This path never creates placeholder files.
pub async fn provider_list(
    kind: &str,
    connection_id: &str,
    relative: &str,
    cursor: Option<&str>,
    limit: usize,
) -> Result<serde_json::Value, String> {
    if relative.starts_with('/')
        || relative.split('/').any(|segment| {
            segment == ".." || segment == "." || segment.contains('\\') || segment.contains('\0')
        })
    {
        return Err("invalid remote relative path".into());
    }
    let conn = load_stored()
        .into_iter()
        .find(|c| c.id == connection_id && c.kind == kind)
        .ok_or("connection not found")?;
    // The manifest and actual user-granted permission gate every request.
    // No built-in provider is trusted implicitly, including the official ones.
    let plugin_id = ensure_provider_access(&conn)?;
    let config = provider_config(&conn);
    let metadata = load_meta(&conn);
    let node_id = metadata
        .files
        .get(relative)
        .map(|m| m.node_id.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(relative);
    crate::plugins::host::global().invoke(
        &plugin_id,
        "fs.list",
        serde_json::json!({
            "mountId": conn.mount_id,
            "connection": config,
            "nodeId": node_id,
            "cursor": cursor.unwrap_or("0"),
            "limit": limit.clamp(1, 500)
        }),
    )
}
