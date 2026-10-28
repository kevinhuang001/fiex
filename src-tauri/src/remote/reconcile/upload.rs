use super::*;

/// Push one immutable snapshot through `vfs.writeStream` in protocol-sized
/// chunks. `offset` is always the number of bytes already accepted by the
/// provider.
pub(super) fn stream_snapshot(
    plugin: &str,
    mount_id: &str,
    stream: &str,
    path: &std::path::Path,
    size: u64,
) -> Result<Value, String> {
    use base64::Engine;
    use std::io::Read;

    const CHUNK: usize = 256 * 1024;
    let mut snapshot = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut buffer = vec![0u8; CHUNK];
    let mut offset = 0u64;
    loop {
        let read = snapshot.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        let base64 = base64::engine::general_purpose::STANDARD.encode(&buffer[..read]);
        let response = crate::plugins::host::global().invoke(
            plugin,
            "vfs.writeStream",
            json!({
                "mountId": mount_id,
                "streamId": stream,
                "offset": offset,
                "base64": base64
            }),
        )?;
        let written = response
            .get("written")
            .and_then(Value::as_u64)
            .unwrap_or(read as u64);
        if written != read as u64 {
            return Err("provider acknowledged a short write".into());
        }
        offset += read as u64;
    }
    if offset != size {
        return Err("upload snapshot changed during transfer".into());
    }
    crate::plugins::host::global().invoke(
        plugin,
        "vfs.commit",
        json!({"mountId": mount_id, "streamId": stream}),
    )
}

pub(super) fn upload(
    conn: &StoredConnection,
    rel: &str,
    file: &RemoteMetaFile,
) -> Result<String, String> {
    let local = local_for(conn, rel);
    check_cache_path(conn, &local)?;
    // A stable disk snapshot prevents mixing two saves during a long upload.
    let mut snapshot = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
    let before = fingerprint(&local)?;
    std::io::copy(
        &mut fs::File::open(&local).map_err(|e| e.to_string())?,
        &mut snapshot,
    )
    .map_err(|e| e.to_string())?;
    if fingerprint(snapshot.path())? != before || fingerprint(&local)? != before {
        return Err(crate::i18n::t("文件仍在保存，稍后重试").into());
    }
    if file.revision.is_empty() {
        return Err(crate::i18n::t("提供者不支持版本条件写入，修改已保留在本地").into());
    }
    let size = snapshot
        .as_file()
        .metadata()
        .map_err(|e| e.to_string())?
        .len();
    let plugin = ensure_provider_access(conn)?;
    let node = if file.node_id.is_empty() {
        rel
    } else {
        &file.node_id
    };
    // Core streams the bytes: providers never receive a snapshot ref and run
    // no Operation Scope for uploads.
    let started = invoke(
        conn,
        "vfs.beginWrite",
        json!({
            "nodeId": node,
            "size": size,
            "ifRevision": file.revision,
        }),
    )?;
    let stream_id = started
        .get("streamId")
        .and_then(Value::as_str)
        .ok_or("provider returned no write stream")?
        .to_string();
    let mut lease = WriteLease {
        plugin: &plugin,
        mount_id: &conn.mount_id,
        stream: &stream_id,
        finished: false,
    };
    let committed = stream_snapshot(&plugin, &conn.mount_id, &stream_id, snapshot.path(), size)?;
    let revision = committed["revision"]
        .as_str()
        .filter(|revision| !revision.is_empty())
        .ok_or("missing committed revision")?
        .to_string();
    lease.finished = true;
    let mut meta = load_meta(conn);
    let entry = meta.files.get_mut(rel).ok_or("cache entry disappeared")?;
    entry.revision = revision.clone();
    entry.baseline = before;
    entry.local_stamp = if fingerprint(&local)? == entry.baseline {
        stamp(&local)?
    } else {
        String::new()
    };
    entry.size = size;
    entry.conflict = false;
    entry.uploaded = true;
    save_meta(conn, &meta)?;
    Ok(revision)
}
