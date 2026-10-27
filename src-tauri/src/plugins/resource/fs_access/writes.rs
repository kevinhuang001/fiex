use super::*;

pub(super) struct PendingWrite {
    pub(super) owner: String,
    pub(super) root: Dir,
    pub(super) root_path: PathBuf,
    pub(super) path: PathBuf,
    pub(super) original: Option<Fingerprint>,
    pub(super) staging: tempfile::NamedTempFile,
}
pub fn writer_permission(owner: &str, handle: &str) -> Result<&'static str, String> {
    let pending = writes().lock().map_err(|_| "write registry unavailable")?;
    let write = pending
        .get(handle)
        .filter(|v| v.owner == owner)
        .ok_or("write handle unavailable")?;
    Ok(if write.original.is_some() {
        "files.write"
    } else {
        "files.create"
    })
}

/// Cheap conflict detector: a full content hash costs a second read of a large
/// file on begin, commit and abort. Size plus modification time catches the
/// realistic concurrent changes without blocking on hundreds of gigabytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Fingerprint {
    pub(super) len: u64,
    pub(super) modified: Option<cap_std::time::SystemTime>,
    pub(super) readonly: bool,
    /// Device and inode (or volume/file index) catch replacement by a file
    /// that happens to keep the same size and timestamp.
    pub(super) identity: Option<(u64, u64)>,
}
pub(super) fn writes() -> &'static Mutex<HashMap<String, PendingWrite>> {
    static VALUES: OnceLock<Mutex<HashMap<String, PendingWrite>>> = OnceLock::new();
    VALUES.get_or_init(|| Mutex::new(HashMap::new()))
}
/// Best-effort identity of the opened target. It never authorizes access; it
/// only makes concurrent-replacement conflicts more likely to be detected.
#[cfg(unix)]
pub(super) fn identity(dir: &Dir, path: &Path) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    let metadata = dir.open(path).ok()?.into_std().metadata().ok()?;
    Some((metadata.dev(), metadata.ino()))
}

#[cfg(windows)]
pub(super) fn identity(dir: &Dir, path: &Path) -> Option<(u64, u64)> {
    use std::os::windows::fs::MetadataExt;
    // `file_index` is still unstable on Windows; creation + write time is the
    // strongest stable signal available without an extra Win32 dependency.
    let metadata = dir.open(path).ok()?.into_std().metadata().ok()?;
    Some((metadata.creation_time(), metadata.last_write_time()))
}

#[cfg(not(any(unix, windows)))]
pub(super) fn identity(_dir: &Dir, _path: &Path) -> Option<(u64, u64)> {
    None
}

/// Conflict detection covers size, modification time, readonly state and file
/// identity. It intentionally does not read the whole file on every commit, so
/// a same-size edit that also preserves all of those metadata fields is not
/// detected; the atomic rename still prevents partial files.
pub(super) fn fingerprint(dir: &Dir, path: &Path) -> Result<Option<Fingerprint>, String> {
    match dir.metadata(path) {
        Ok(metadata) => Ok(Some(Fingerprint {
            len: metadata.len(),
            modified: metadata.modified().ok(),
            readonly: metadata.permissions().readonly(),
            identity: identity(dir, path),
        })),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}
pub fn begin_write(owner: &str, root: &Scope, input: &str, create: bool) -> Result<String, String> {
    let dir = directory(root)?;
    let path = relative(root, input)?;
    let original = fingerprint(&dir, &path)?;
    if create == original.is_some() {
        return Err(if create {
            "file already exists"
        } else {
            "file does not exist"
        }
        .into());
    }
    let mut pending = writes().lock().map_err(|_| "write registry unavailable")?;
    if pending.values().filter(|v| v.owner == owner).count() >= MAX_HANDLES {
        return Err("too many open writes".into());
    }
    let staging = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
    let handle = format!(
        "write-{}",
        staging
            .path()
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
    );
    pending.insert(
        handle.clone(),
        PendingWrite {
            owner: owner.into(),
            root: dir,
            root_path: root.path().to_path_buf(),
            path,
            original,
            staging,
        },
    );
    Ok(handle)
}
pub fn write_chunk(owner: &str, handle: &str, encoded: &str) -> Result<(), String> {
    if encoded.len() > MAX_CHUNK.div_ceil(3) * 4 {
        return Err("write chunk too large".into());
    }
    let bytes = STANDARD.decode(encoded).map_err(|e| e.to_string())?;
    write_bytes(owner, handle, &bytes)
}
pub fn write_buffer(owner: &str, handle: &str, buffer: &str, size: usize) -> Result<(), String> {
    if size > MAX_CHUNK {
        return Err("write chunk too large".into());
    }
    let bytes = crate::plugins::ipc::read(owner, buffer)?;
    if size > bytes.len() {
        return Err("write buffer too small".into());
    }
    write_bytes(owner, handle, &bytes[..size])
}
pub(super) fn write_bytes(owner: &str, handle: &str, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MAX_CHUNK {
        return Err("write chunk too large".into());
    }
    let mut pending = writes().lock().map_err(|_| "write registry unavailable")?;
    let write = pending
        .get_mut(handle)
        .filter(|v| v.owner == owner)
        .ok_or("write handle unavailable")?;
    write.staging.write_all(bytes).map_err(|e| e.to_string())
}
pub fn abort(owner: &str, handle: &str) -> Result<(), String> {
    let mut pending = writes().lock().map_err(|_| "write registry unavailable")?;
    // Aborting an already-failed commit is a successful cleanup: the commit
    // path removes the transaction before doing expensive work.
    match pending.get(handle) {
        Some(value) if value.owner == owner => {
            pending.remove(handle);
            Ok(())
        }
        None => Ok(()),
        Some(_) => Err("write handle unavailable".into()),
    }
}
#[cfg(windows)]
pub(super) fn replace_file(dir: &Dir, from: &str, to: &str) -> std::io::Result<()> {
    // Windows can transiently deny replacing a file while a scanner or a
    // closing handle still holds it. The rename itself is atomic, so a short
    // retry is safe and avoids failing an otherwise valid commit.
    let mut last = None;
    for _ in 0..8 {
        match dir.rename(from, dir, to) {
            Ok(()) => return Ok(()),
            Err(error) => {
                last = Some(error);
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
        }
    }
    Err(last.unwrap_or_else(|| std::io::Error::other("replace failed")))
}
#[cfg(not(windows))]
pub(super) fn replace_file(dir: &Dir, from: &str, to: &str) -> std::io::Result<()> {
    dir.rename(from, dir, to)
}
pub fn commit(owner: &str, handle: &str) -> Result<Value, String> {
    // Remove the transaction before touching the target so other independent
    // commits never wait for a large copy or fsync.
    let mut write = {
        let mut pending = writes().lock().map_err(|_| "write registry unavailable")?;
        if pending.get(handle).is_none_or(|value| value.owner != owner) {
            return Err("write handle unavailable".into());
        }
        pending.remove(handle).ok_or("write handle unavailable")?
    };
    let dir = &write.root;
    if fingerprint(dir, &write.path)? != write.original {
        return Err("file changed since write began; commit aborted".into());
    }
    let parent = write
        .path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = write.path.file_name().ok_or("invalid write path")?;
    let target_dir = dir.open_dir(parent).map_err(|e| e.to_string())?;
    let temp_name = format!(
        ".fiex-commit-{}",
        write
            .staging
            .path()
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
    );
    write
        .staging
        .as_file()
        .sync_all()
        .map_err(|e| e.to_string())?;
    let mut temp_exists = false;
    let result = (|| {
        // Fast path: when the staging file and the target live on the same
        // filesystem, an atomic rename avoids copying the whole file. Only use
        // it while the captured root still resolves to the same directory; a
        // replaced root path falls back to the capability copy below.
        let mut moved = false;
        if let Ok(canonical_parent) = dir.canonicalize(parent) {
            let absolute_parent = write.root_path.join(&canonical_parent);
            let root_is_intact = dunce::canonicalize(&write.root_path)
                .map(|path| path == write.root_path)
                .unwrap_or(false);
            if root_is_intact
                && std::fs::rename(write.staging.path(), absolute_parent.join(&temp_name)).is_ok()
            {
                moved = true;
            }
        }
        if moved {
            temp_exists = true;
        } else {
            let mut output = target_dir
                .open_with(
                    &temp_name,
                    OpenOptions::new().read(true).write(true).create_new(true),
                )
                .map_err(|e| format!("create commit temp: {e}"))?;
            temp_exists = true;
            write
                .staging
                .seek(SeekFrom::Start(0))
                .map_err(|e| e.to_string())?;
            std::io::copy(&mut write.staging, &mut output)
                .map_err(|e| format!("copy commit temp: {e}"))?;
            output.sync_all().map_err(|e| e.to_string())?;
            // Windows refuses to replace a file while this handle is open.
            drop(output);
        }
        if write.original.is_some() {
            let permissions = dir
                .metadata(&write.path)
                .map_err(|e| e.to_string())?
                .permissions();
            let temp = target_dir
                .open_with(&temp_name, OpenOptions::new().read(true).write(true))
                .map_err(|e| format!("open commit temp: {e}"))?;
            temp.set_permissions(permissions)
                .map_err(|e| format!("set commit permissions: {e}"))?;
            temp.sync_all().map_err(|e| e.to_string())?;
        }
        if fingerprint(dir, &write.path)? != write.original {
            return Err("file changed during commit".into());
        }
        if write.original.is_none() {
            // link provides atomic create-if-absent without overwriting a racer.
            target_dir
                .hard_link(&temp_name, &target_dir, name)
                .map_err(|e| e.to_string())?;
            target_dir
                .remove_file(&temp_name)
                .map_err(|e| e.to_string())?;
        } else {
            replace_file(&target_dir, &temp_name, &name.to_string_lossy())
                .map_err(|e| format!("replace commit target: {e}"))?;
        }
        entry(dir, &write.path)
    })();
    if result.is_err() && temp_exists {
        let _ = target_dir.remove_file(&temp_name);
    }
    result
}
pub fn seek_write(owner: &str, handle: &str, kind: &str, offset: i64) -> Result<u64, String> {
    let mut pending = writes().lock().map_err(|_| "write registry unavailable")?;
    let write = pending
        .get_mut(handle)
        .filter(|v| v.owner == owner)
        .ok_or("write handle unavailable")?;
    let position = match kind {
        "start" if offset >= 0 => SeekFrom::Start(offset as u64),
        "current" => SeekFrom::Current(offset),
        "end" => SeekFrom::End(offset),
        _ => return Err("invalid write seek".into()),
    };
    write.staging.seek(position).map_err(|e| e.to_string())
}
