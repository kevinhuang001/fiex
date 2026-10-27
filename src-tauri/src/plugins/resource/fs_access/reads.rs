use super::*;

pub(super) struct PendingRead {
    pub(super) owner: String,
    pub(super) file: cap_std::fs::File,
    pub(super) length: u64,
    pub(super) _token: tempfile::NamedTempFile,
}
pub(super) fn reads() -> &'static Mutex<HashMap<String, PendingRead>> {
    static VALUES: OnceLock<Mutex<HashMap<String, PendingRead>>> = OnceLock::new();
    VALUES.get_or_init(|| Mutex::new(HashMap::new()))
}
pub fn open_read(owner: &str, root: &Scope, input: &str) -> Result<Value, String> {
    let file = directory(root)?
        .open(relative(root, input)?)
        .map_err(|e| e.to_string())?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() {
        return Err("expected a file".into());
    }
    let mut reads = reads().lock().map_err(|_| "read registry unavailable")?;
    if reads.values().filter(|v| v.owner == owner).count() >= MAX_HANDLES {
        return Err("too many open reads".into());
    }
    let token = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
    let handle = format!(
        "read-{}",
        token
            .path()
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
    );
    reads.insert(
        handle.clone(),
        PendingRead {
            owner: owner.into(),
            file,
            length: metadata.len(),
            _token: token,
        },
    );
    Ok(json!({"handle": handle, "size": metadata.len()}))
}
/// Duplicate only a reader owned by the calling operation/task. The network
/// layer streams this authorized handle and never accepts a filesystem path.
pub fn http_body(owner: &str, handle: &str) -> Result<std::fs::File, String> {
    let registry = reads().lock().map_err(|_| "read registry unavailable")?;
    let reader = registry
        .get(handle)
        .filter(|r| r.owner == owner)
        .ok_or("read handle unavailable")?;
    let mut file = reader
        .file
        .try_clone()
        .map_err(|e| e.to_string())?
        .into_std();
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    Ok(file)
}

pub fn read_open(
    owner: &str,
    handle: &str,
    offset: u64,
    length: usize,
    buffer: Option<&str>,
) -> Result<Value, String> {
    let mut reads = reads().lock().map_err(|_| "read registry unavailable")?;
    let read = reads
        .get_mut(handle)
        .filter(|v| v.owner == owner)
        .ok_or("read handle unavailable")?;
    read.file
        .seek(SeekFrom::Start(offset))
        .map_err(|e| e.to_string())?;
    let remaining = read.length.saturating_sub(offset);
    let capacity = length
        .min(if buffer.is_some() {
            4 * 1024 * 1024
        } else {
            MAX_CHUNK
        })
        .min(usize::try_from(remaining).unwrap_or(usize::MAX));
    if let Some(id) = buffer {
        let size = crate::plugins::ipc::read_file_into(owner, id, &mut read.file, capacity)?;
        Ok(json!({"size": size, "eof": offset.saturating_add(size as u64) >= read.length}))
    } else {
        let mut bytes = vec![0; capacity];
        let size = read.file.read(&mut bytes).map_err(|e| e.to_string())?;
        bytes.truncate(size);
        Ok(
            json!({"base64": STANDARD.encode(&bytes), "size": size, "eof": offset.saturating_add(size as u64) >= read.length}),
        )
    }
}
pub fn close_read(owner: &str, handle: &str) -> Result<(), String> {
    let mut reads = reads().lock().map_err(|_| "read registry unavailable")?;
    if reads.get(handle).is_none_or(|v| v.owner != owner) {
        return Err("read handle unavailable".into());
    }
    reads.remove(handle);
    Ok(())
}
