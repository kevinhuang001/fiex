//! Host-owned, directory-confined file access. Directory handles prevent
//! symlink replacement from escaping the captured task root.
use base64::{engine::general_purpose::STANDARD, Engine};
use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
    sync::{Mutex, OnceLock},
};
const MAX_CHUNK: usize = 256 * 1024;
const MAX_HANDLES: usize = 32;

struct PendingRead {
    owner: String,
    file: cap_std::fs::File,
    length: u64,
    _token: tempfile::NamedTempFile,
}
fn reads() -> &'static Mutex<HashMap<String, PendingRead>> {
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
        let size = crate::plugin_ipc::read_file_into(owner, id, &mut read.file, capacity)?;
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

/// A save dialog may select a file that does not exist yet. Canonicalize its
/// existing parent so a symlinked parent cannot change the selected target.
pub fn canonical_selection(path: &Path) -> Result<PathBuf, String> {
    match dunce::canonicalize(path) {
        Ok(path) => Ok(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = path.parent().ok_or("selection has no parent")?;
            let name = path.file_name().ok_or("selection has no name")?;
            Ok(dunce::canonicalize(parent)
                .map_err(|e| e.to_string())?
                .join(name))
        }
        Err(error) => Err(error.to_string()),
    }
}

/// Captured once at invocation start, including an open directory capability.
#[derive(Clone)]
pub struct Scope {
    path: PathBuf,
    dir: std::sync::Arc<Dir>,
    allowed: Option<Vec<(PathBuf, bool)>>,
    references: HashMap<String, String>,
    metadata_only: bool,
    metadata_refs: HashMap<String, Value>,
}
impl Scope {
    pub fn new(path: &Path) -> Result<Self, String> {
        let path = dunce::canonicalize(path).map_err(|e| e.to_string())?;
        let dir = Dir::open_ambient_dir(&path, ambient_authority()).map_err(|e| e.to_string())?;
        Ok(Self {
            path,
            dir: std::sync::Arc::new(dir),
            allowed: None,
            references: HashMap::new(),
            metadata_only: false,
            metadata_refs: HashMap::new(),
        })
    }
    /// Capture only paths chosen for this operation. A file grants that file;
    /// a directory grants its descendants. An empty selection grants no files.
    pub fn selected(path: &Path, paths: &[PathBuf]) -> Result<Self, String> {
        let mut scope = Self::new(path)?;
        let mut allowed = Vec::with_capacity(paths.len());
        for selected in paths {
            let canonical = canonical_selection(selected)?;
            let relative = canonical
                .strip_prefix(&scope.path)
                .map_err(|_| "selection outside task directory")?
                .to_path_buf();
            let relative = if relative.as_os_str().is_empty() {
                PathBuf::from(".")
            } else {
                relative
            };
            let is_dir = scope.dir.metadata(&relative).is_ok_and(|m| m.is_dir());
            allowed.push((relative, is_dir));
        }
        scope.allowed = Some(allowed);
        Ok(scope)
    }
    pub fn restrict_to_metadata(&mut self) {
        self.metadata_only = true;
    }
    pub fn authorize_method(&self, method: &str) -> Result<(), String> {
        if self.metadata_only
            && ((method.starts_with("host.fs.")
                && !matches!(method, "host.fs.stat" | "host.fs.statBatch"))
                || matches!(method, "host.process.run" | "host.exec"))
        {
            return Err("menu resolution grants metadata only".into());
        }
        Ok(())
    }
    pub fn selected_paths(&self) -> Vec<PathBuf> {
        self.allowed
            .as_ref()
            .map(|paths| paths.iter().map(|(p, _)| self.path.join(p)).collect())
            .unwrap_or_default()
    }
    /// Remote metadata is operation-owned; it never confers content access.
    pub fn bind_metadata(&mut self, reference: &Value, metadata: Value) -> Result<(), String> {
        let reference: crate::plugin_contracts::ResourceRef =
            serde_json::from_value(reference.clone()).map_err(|e| e.to_string())?;
        let key = serde_json::to_string(&reference).map_err(|e| e.to_string())?;
        self.metadata_refs.insert(key, metadata);
        Ok(())
    }
    pub fn metadata_reference(&self, reference: &Value) -> Result<Option<&Value>, String> {
        let reference: crate::plugin_contracts::ResourceRef =
            serde_json::from_value(reference.clone()).map_err(|e| e.to_string())?;
        let key = serde_json::to_string(&reference).map_err(|e| e.to_string())?;
        Ok(self.metadata_refs.get(&key))
    }
    pub fn bind_reference(
        &mut self,
        reference: &crate::plugin_contracts::ResourceRef,
        path: &Path,
    ) -> Result<(), String> {
        let path = path.to_string_lossy().into_owned();
        self.authorize(&path)?;
        let key = serde_json::to_string(reference).map_err(|e| e.to_string())?;
        self.references.insert(key, path);
        Ok(())
    }
    pub fn resolve_reference(&self, reference: &serde_json::Value) -> Result<&str, String> {
        let reference: crate::plugin_contracts::ResourceRef =
            serde_json::from_value(reference.clone()).map_err(|e| e.to_string())?;
        let key = serde_json::to_string(&reference).map_err(|e| e.to_string())?;
        self.references
            .get(&key)
            .map(String::as_str)
            .ok_or_else(|| "resource not selected for this operation".into())
    }
    pub fn authorize(&self, path: &str) -> Result<(), String> {
        relative(self, path).map(|_| ())
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn relative(root: &Scope, input: &str) -> Result<PathBuf, String> {
    let path = Path::new(input);
    let path = if path.is_absolute() {
        path.strip_prefix(&root.path)
            .map_err(|_| "path outside task directory")?
    } else {
        path
    };
    if path
        .components()
        .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
    {
        return Err("path outside task directory".into());
    }
    let path = if path.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        path.into()
    };
    if let Some(allowed) = &root.allowed {
        let resolved = match root.dir.canonicalize(&path) {
            Ok(path) => path,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let parent = path
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or(Path::new("."));
                root.dir
                    .canonicalize(parent)
                    .map_err(|e| e.to_string())?
                    .join(path.file_name().ok_or("invalid selected path")?)
            }
            Err(error) => return Err(error.to_string()),
        };
        let normalize = |candidate: &Path| {
            let normalized: PathBuf = candidate
                .components()
                .filter(|part| matches!(part, Component::Normal(_)))
                .collect();
            if normalized.as_os_str().is_empty() {
                PathBuf::from(".")
            } else {
                normalized
            }
        };
        let resolved = normalize(&resolved);
        let permitted = |candidate: &Path| {
            allowed.iter().any(|(selected, is_dir)| {
                candidate == selected
                    || (*is_dir && (selected == Path::new(".") || candidate.starts_with(selected)))
            })
        };
        if !permitted(&path) || !permitted(&resolved) {
            return Err("path not selected for this operation".into());
        }
    }
    Ok(path)
}
fn directory(root: &Scope) -> Result<Dir, String> {
    root.dir.try_clone().map_err(|e| e.to_string())
}
fn entry(dir: &Dir, path: &Path) -> Result<Value, String> {
    let metadata = dir.metadata(path).map_err(|e| e.to_string())?;
    Ok(
        json!({"path": path.to_string_lossy(), "name": path.file_name().unwrap_or_default().to_string_lossy(), "isDir": metadata.is_dir(), "size": metadata.len(), "modified": metadata.modified().ok().and_then(|t| t.into_std().duration_since(std::time::UNIX_EPOCH).ok()).map(|t| t.as_secs()).unwrap_or(0), "extension": path.extension().unwrap_or_default().to_string_lossy(), "mime": if metadata.is_dir() { "inode/directory".into() } else { mime_guess::from_path(path).first_or_octet_stream().to_string() }, "readonly": metadata.permissions().readonly(), "symlink": dir.symlink_metadata(path).map(|m| m.is_symlink()).unwrap_or(false)}),
    )
}
pub fn list(root: &Scope, input: &str) -> Result<Value, String> {
    let dir = directory(root)?;
    let path = relative(root, input)?;
    let mut values = Vec::new();
    for item in dir.read_dir(&path).map_err(|e| e.to_string())?.take(5001) {
        if values.len() == 5000 {
            return Err("directory exceeds listing limit".into());
        }
        let item = item.map_err(|e| e.to_string())?;
        // Omit symlinks whose targets cannot be resolved inside the capability.
        let child = path.join(item.file_name());
        if let Ok(value) = entry(&dir, &child) {
            values.push(value);
        }
    }
    values.sort_by_key(|v| v["name"].as_str().unwrap_or("").to_lowercase());
    Ok(json!(values))
}
pub fn stat(root: &Scope, input: &str) -> Result<Value, String> {
    entry(&directory(root)?, &relative(root, input)?)
}
pub fn is_directory(root: &Scope, input: &str) -> Result<bool, String> {
    let path = relative(root, input)?;
    let metadata = directory(root)?
        .symlink_metadata(path)
        .map_err(|e| e.to_string())?;
    Ok(metadata.is_dir())
}
/// Preserve input order and isolate failures so one inaccessible selection does
/// not disclose its attributes or prevent metadata for the other selections.
pub fn stat_batch(root: &Scope, paths: &[Value]) -> Result<Value, String> {
    if paths.len() > 256 {
        return Err("stat batch exceeds 256 entries".into());
    }
    let dir = directory(root)?;
    let mut seen = std::collections::HashSet::new();
    let results: Vec<Value> = paths
        .iter()
        .map(|value| {
            let Some(path) = value.as_str() else {
                return json!({"error": "invalid path"});
            };
            let Ok(relative) = relative(root, path) else {
                return json!({"error": "permission denied"});
            };
            if !seen.insert(relative.clone()) {
                return json!({"error": "duplicate selection"});
            }
            match entry(&dir, &relative) {
                Ok(metadata) => json!({"metadata": metadata}),
                Err(_) => json!({"error": "metadata unavailable"}),
            }
        })
        .collect();
    Ok(json!({"results": results}))
}
pub fn create(root: &Scope, input: &str) -> Result<Value, String> {
    let dir = directory(root)?;
    let path = relative(root, input)?;
    dir.open_with(
        &path,
        OpenOptions::new().read(true).write(true).create_new(true),
    )
    .map_err(|e| e.to_string())?;
    entry(&dir, &path)
}
pub fn mkdir(root: &Scope, input: &str) -> Result<Value, String> {
    let dir = directory(root)?;
    let path = relative(root, input)?;
    dir.create_dir_all(&path).map_err(|e| e.to_string())?;
    entry(&dir, &path)
}
pub fn read_chunk(root: &Scope, input: &str, offset: u64, length: usize) -> Result<Value, String> {
    let mut file = directory(root)?
        .open(relative(root, input)?)
        .map_err(|e| e.to_string())?;
    let total = file.metadata().map_err(|e| e.to_string())?.len();
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| e.to_string())?;
    let mut data = vec![0; length.min(MAX_CHUNK)];
    let read = file.read(&mut data).map_err(|e| e.to_string())?;
    data.truncate(read);
    Ok(
        json!({"base64": STANDARD.encode(&data), "size": read, "eof": offset.saturating_add(read as u64) >= total}),
    )
}
pub fn read(
    owner: &str,
    root: &Scope,
    input: &str,
    offset: u64,
    length: usize,
) -> Result<Value, String> {
    let value = read_chunk(root, input, offset, length)?;
    let data = STANDARD
        .decode(value["base64"].as_str().unwrap_or(""))
        .map_err(|e| e.to_string())?;
    let buffer = crate::plugin_ipc::create(owner, data.len().max(1), false)?;
    crate::plugin_ipc::write(&buffer.id, &data)?;
    Ok(json!({"buffer": buffer, "size": data.len(), "eof": value["eof"]}))
}
pub fn remove(root: &Scope, input: &str) -> Result<(), String> {
    let path = relative(root, input)?;
    if !path.components().any(|c| matches!(c, Component::Normal(_))) {
        return Err("cannot remove task root".into());
    }
    let dir = directory(root)?;
    let metadata = dir.symlink_metadata(&path).map_err(|e| e.to_string())?;
    if metadata.is_dir() {
        dir.remove_dir_all(path).map_err(|e| e.to_string())
    } else {
        dir.remove_file(path).map_err(|e| e.to_string())
    }
}
pub fn rename(root: &Scope, from: &str, to: &str) -> Result<(), String> {
    let dir = directory(root)?;
    let from = relative(root, from)?;
    let to = relative(root, to)?;
    if !from.components().any(|c| matches!(c, Component::Normal(_)))
        || !to.components().any(|c| matches!(c, Component::Normal(_)))
    {
        return Err("cannot rename task root".into());
    }
    if dir.symlink_metadata(&to).is_ok() {
        return Err("rename destination already exists".into());
    }
    dir.rename(from, &dir, to).map_err(|e| e.to_string())
}

struct PendingWrite {
    owner: String,
    root: Dir,
    root_path: PathBuf,
    path: PathBuf,
    original: Option<Fingerprint>,
    staging: tempfile::NamedTempFile,
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
struct Fingerprint {
    len: u64,
    modified: Option<cap_std::time::SystemTime>,
    readonly: bool,
    /// Device and inode (or volume/file index) catch replacement by a file
    /// that happens to keep the same size and timestamp.
    identity: Option<(u64, u64)>,
}
fn writes() -> &'static Mutex<HashMap<String, PendingWrite>> {
    static VALUES: OnceLock<Mutex<HashMap<String, PendingWrite>>> = OnceLock::new();
    VALUES.get_or_init(|| Mutex::new(HashMap::new()))
}
/// Best-effort identity of the opened target. It never authorizes access; it
/// only makes concurrent-replacement conflicts more likely to be detected.
#[cfg(unix)]
fn identity(dir: &Dir, path: &Path) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    let metadata = dir.open(path).ok()?.into_std().metadata().ok()?;
    Some((metadata.dev(), metadata.ino()))
}

#[cfg(windows)]
fn identity(dir: &Dir, path: &Path) -> Option<(u64, u64)> {
    use std::os::windows::fs::MetadataExt;
    // `file_index` is still unstable on Windows; creation + write time is the
    // strongest stable signal available without an extra Win32 dependency.
    let metadata = dir.open(path).ok()?.into_std().metadata().ok()?;
    Some((metadata.creation_time(), metadata.last_write_time()))
}

#[cfg(not(any(unix, windows)))]
fn identity(_dir: &Dir, _path: &Path) -> Option<(u64, u64)> {
    None
}

/// Conflict detection covers size, modification time, readonly state and file
/// identity. It intentionally does not read the whole file on every commit, so
/// a same-size edit that also preserves all of those metadata fields is not
/// detected; the atomic rename still prevents partial files.
fn fingerprint(dir: &Dir, path: &Path) -> Result<Option<Fingerprint>, String> {
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
    let bytes = crate::plugin_ipc::read(owner, buffer)?;
    if size > bytes.len() {
        return Err("write buffer too small".into());
    }
    write_bytes(owner, handle, &bytes[..size])
}
fn write_bytes(owner: &str, handle: &str, bytes: &[u8]) -> Result<(), String> {
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
fn replace_file(dir: &Dir, from: &str, to: &str) -> std::io::Result<()> {
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
fn replace_file(dir: &Dir, from: &str, to: &str) -> std::io::Result<()> {
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
pub fn write(
    owner: &str,
    root: &Scope,
    path: &str,
    buffer: &str,
    mode: &str,
) -> Result<Value, String> {
    let bytes = crate::plugin_ipc::read(owner, buffer)?;
    let handle = begin_write(owner, root, path, mode == "create")?;
    let result = (|| {
        for chunk in bytes.chunks(MAX_CHUNK) {
            write_chunk(owner, &handle, &STANDARD.encode(chunk))?;
        }
        commit(owner, &handle)
    })();
    if result.is_err() {
        let _ = abort(owner, &handle);
    }
    result
}
pub fn cleanup(owner: &str) {
    if let Ok(mut pending) = reads().lock() {
        pending.retain(|_, v| v.owner != owner);
    }
    if let Ok(mut pending) = writes().lock() {
        pending.retain(|_, v| v.owner != owner);
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn open_read_keeps_the_selected_file_and_supports_sparse_large_offsets() {
        let root = tempfile::tempdir().unwrap();
        let chosen = root.path().join("chosen");
        let mut file = std::fs::File::create(&chosen).unwrap();
        let far = 5_u64 * 1024 * 1024 * 1024;
        file.seek(SeekFrom::Start(far)).unwrap();
        file.write_all(&[0, 255]).unwrap();
        let scope = Scope::selected(root.path(), std::slice::from_ref(&chosen)).unwrap();
        let value = open_read("owner", &scope, "chosen").unwrap();
        let handle = value["handle"].as_str().unwrap();
        assert_eq!(value["size"], far + 2);
        assert!(http_body("other", handle).is_err());
        let mut upload = http_body("owner", handle).unwrap();
        assert_eq!(upload.metadata().unwrap().len(), far + 2);
        assert_eq!(upload.stream_position().unwrap(), 0);
        assert!(read_open("other", handle, far, 2, None).is_err());
        std::fs::rename(&chosen, root.path().join("moved")).unwrap();
        std::fs::write(&chosen, b"replacement").unwrap();
        let data = read_open("owner", handle, far, 2, None).unwrap();
        assert_eq!(data["base64"], STANDARD.encode([0, 255]));
        let descriptor = crate::plugin_ipc::create("owner", 4096, false).unwrap();
        assert!(read_open("other", handle, far, 2, Some(&descriptor.id)).is_err());
        assert!(read_open("owner", handle, far, 2, Some(&descriptor.id)).is_ok());
        assert_eq!(
            &crate::plugin_ipc::read("owner", &descriptor.id).unwrap()[..2],
            &[0, 255]
        );
        crate::plugin_ipc::release("owner", &descriptor.id).unwrap();
        close_read("owner", handle).unwrap();
        assert!(http_body("owner", handle).is_err());
        assert!(read_open("owner", handle, 0, 1, None).is_err());
    }
    #[test]
    fn operation_selection_does_not_expose_sibling_files() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("chosen"), b"ok").unwrap();
        std::fs::write(root.path().join("private"), b"secret").unwrap();
        let scope = Scope::selected(root.path(), &[root.path().join("chosen")]).unwrap();
        assert!(read_chunk(&scope, "chosen", 0, 8).is_ok());
        assert!(read_chunk(&scope, "private", 0, 8).is_err());
        assert!(list(&scope, ".").is_err());
        assert!(stat(&scope, "private").is_err());
        let empty = Scope::selected(root.path(), &[]).unwrap();
        assert!(read_chunk(&empty, "chosen", 0, 8).is_err());
    }
    #[test]
    fn batch_stat_keeps_order_and_hides_unselected_entries() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("chosen"), b"ok").unwrap();
        std::fs::write(root.path().join("private"), b"secret").unwrap();
        let scope = Scope::selected(root.path(), &[root.path().join("chosen")]).unwrap();
        let result = stat_batch(
            &scope,
            &[
                json!("chosen"),
                json!("private"),
                json!("chosen"),
                json!(null),
            ],
        )
        .unwrap();
        assert_eq!(result["results"][0]["metadata"]["name"], "chosen");
        assert!(result["results"][1].get("metadata").is_none());
        assert_eq!(result["results"][1]["error"], "permission denied");
        assert_eq!(result["results"][2]["error"], "duplicate selection");
        assert_eq!(result["results"][3]["error"], "invalid path");
        assert!(stat_batch(&scope, &vec![json!("chosen"); 257]).is_err());
    }
    #[test]
    fn selected_directory_allows_descendants_only() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("chosen")).unwrap();
        std::fs::write(root.path().join("chosen/file"), b"ok").unwrap();
        std::fs::write(root.path().join("other"), b"secret").unwrap();
        let scope = Scope::selected(root.path(), &[root.path().join("chosen")]).unwrap();
        assert!(read_chunk(&scope, "chosen/file", 0, 8).is_ok());
        assert!(read_chunk(&scope, "other", 0, 8).is_err());
    }
    #[test]
    fn save_selection_allows_only_the_new_target() {
        let root = tempfile::tempdir().unwrap();
        let scope = Scope::selected(root.path(), &[root.path().join("output.txt")]).unwrap();
        let result = create(&scope, "output.txt");
        assert!(result.is_ok(), "{result:?}");
        assert!(create(&scope, "other.txt").is_err());
    }
    #[cfg(unix)]
    #[test]
    fn selected_folder_cannot_follow_a_link_to_its_sibling() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("chosen")).unwrap();
        std::fs::write(root.path().join("private"), b"secret").unwrap();
        std::os::unix::fs::symlink("../private", root.path().join("chosen/link")).unwrap();
        let scope = Scope::selected(root.path(), &[root.path().join("chosen")]).unwrap();
        assert!(read_chunk(&scope, "chosen/link", 0, 8).is_err());
    }
    #[test]
    fn staged_writes_preserve_conflicts_and_abort() {
        let root = tempfile::tempdir().unwrap();
        let scope = Scope::new(root.path()).unwrap();
        std::fs::write(root.path().join("file"), b"old").unwrap();
        let handle = begin_write("p", &scope, "file", false).unwrap();
        write_chunk("p", &handle, &STANDARD.encode(b"new")).unwrap();
        assert_eq!(std::fs::read(root.path().join("file")).unwrap(), b"old");
        assert!(write_chunk("other", &handle, "").is_err());
        std::fs::write(root.path().join("file"), b"external").unwrap();
        assert!(commit("p", &handle).is_err());
        abort("p", &handle).unwrap();
        assert_eq!(
            std::fs::read(root.path().join("file")).unwrap(),
            b"external"
        );
        let handle = begin_write("p", &scope, "created", true).unwrap();
        write_chunk("p", &handle, &STANDARD.encode([0, 255, 128])).unwrap();
        commit("p", &handle).unwrap();
        assert_eq!(
            std::fs::read(root.path().join("created")).unwrap(),
            [0, 255, 128]
        );
        assert!(remove(&scope, ".").is_err());
    }
    #[test]
    fn shared_write_is_owned_bounded_and_transactional() {
        let root = tempfile::tempdir().unwrap();
        let scope = Scope::selected(root.path(), &[root.path().join("new")]).unwrap();
        let handle = begin_write("owner", &scope, "new", true).unwrap();
        let buffer = crate::plugin_ipc::create("owner", 256 * 1024, true).unwrap();
        crate::plugin_ipc::write_owned("owner", &buffer.id, &[0, 255, 42]).unwrap();
        assert!(write_buffer("other", &handle, &buffer.id, 3).is_err());
        assert!(write_buffer("owner", &handle, &buffer.id, MAX_CHUNK + 1).is_err());
        write_buffer("owner", &handle, &buffer.id, 3).unwrap();
        assert!(!root.path().join("new").exists());
        commit("owner", &handle).unwrap();
        assert_eq!(
            std::fs::read(root.path().join("new")).unwrap(),
            [0, 255, 42]
        );
        crate::plugin_ipc::release("owner", &buffer.id).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn symlinks_cannot_escape_task_root() {
        let root = tempfile::tempdir().unwrap();
        let scope = Scope::new(root.path()).unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret"), "secret").unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("link")).unwrap();
        assert!(read_chunk(&scope, "link/secret", 0, 100).is_err());
        assert!(begin_write("p", &scope, "link/secret", false).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn replacing_the_file_identity_aborts_commit() {
        let root = tempfile::tempdir().unwrap();
        let scope = Scope::new(root.path()).unwrap();
        let path = root.path().join("file");
        std::fs::write(&path, b"old").unwrap();
        let handle = begin_write("owner", &scope, "file", false).unwrap();
        write_chunk("owner", &handle, &STANDARD.encode(b"new")).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::write(&path, b"other").unwrap();
        assert!(commit("owner", &handle).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"other");
    }
    #[test]
    fn a_wrong_owner_cannot_consume_another_operation_transaction() {
        let root = tempfile::tempdir().unwrap();
        let scope = Scope::new(root.path()).unwrap();
        std::fs::write(root.path().join("file"), b"old").unwrap();
        let handle = begin_write("owner", &scope, "file", false).unwrap();
        write_chunk("owner", &handle, &STANDARD.encode(b"new")).unwrap();
        assert!(commit("other", &handle).is_err());
        assert_eq!(std::fs::read(root.path().join("file")).unwrap(), b"old");
        commit("owner", &handle).unwrap();
        assert_eq!(std::fs::read(root.path().join("file")).unwrap(), b"new");
    }
    #[cfg(unix)]
    #[test]
    fn captured_directory_survives_root_path_replacement() {
        let parent = tempfile::tempdir().unwrap();
        let original = parent.path().join("original");
        let moved = parent.path().join("moved");
        let outside = parent.path().join("outside");
        std::fs::create_dir(&original).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(original.join("file"), b"bound").unwrap();
        std::fs::write(outside.join("file"), b"outside").unwrap();
        let scope = Scope::new(&original).unwrap();
        let handle = begin_write("captured", &scope, "file", false).unwrap();
        write_chunk("captured", &handle, &STANDARD.encode(b"updated")).unwrap();
        std::fs::rename(&original, &moved).unwrap();
        std::os::unix::fs::symlink(&outside, &original).unwrap();
        let read = read_chunk(&scope, "file", 0, 100).unwrap();
        assert_eq!(read["base64"], STANDARD.encode(b"bound"));
        commit("captured", &handle).unwrap();
        assert_eq!(std::fs::read(moved.join("file")).unwrap(), b"updated");
        assert_eq!(std::fs::read(outside.join("file")).unwrap(), b"outside");
    }
}
