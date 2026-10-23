//! Host-owned, directory-confined file access. Directory handles prevent
//! symlink replacement from escaping the captured task root.
use base64::{engine::general_purpose::STANDARD, Engine};
use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    io::{Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
    sync::{Mutex, OnceLock},
};
const MAX_CHUNK: usize = 256 * 1024;
const MAX_HANDLES: usize = 32;

/// Captured once at invocation start, including an open directory capability.
#[derive(Clone)]
pub struct Scope {
    path: PathBuf,
    dir: std::sync::Arc<Dir>,
}
impl Scope {
    pub fn new(path: &Path) -> Result<Self, String> {
        let path = dunce::canonicalize(path).map_err(|e| e.to_string())?;
        let dir = Dir::open_ambient_dir(&path, ambient_authority()).map_err(|e| e.to_string())?;
        Ok(Self {
            path,
            dir: std::sync::Arc::new(dir),
        })
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
    Ok(if path.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        path.into()
    })
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
pub fn create(root: &Scope, input: &str) -> Result<Value, String> {
    let dir = directory(root)?;
    let path = relative(root, input)?;
    dir.open_with(&path, OpenOptions::new().write(true).create_new(true))
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
    path: PathBuf,
    original: Option<Vec<u8>>,
    staging: tempfile::NamedTempFile,
}
fn writes() -> &'static Mutex<HashMap<String, PendingWrite>> {
    static VALUES: OnceLock<Mutex<HashMap<String, PendingWrite>>> = OnceLock::new();
    VALUES.get_or_init(|| Mutex::new(HashMap::new()))
}
fn fingerprint(dir: &Dir, path: &Path) -> Result<Option<Vec<u8>>, String> {
    let mut file = match dir.open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let mut hash = Sha256::new();
    let mut block = [0; 65536];
    loop {
        let count = file.read(&mut block).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hash.update(&block[..count]);
    }
    Ok(Some(hash.finalize().to_vec()))
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
    let mut pending = writes().lock().map_err(|_| "write registry unavailable")?;
    let write = pending
        .get_mut(handle)
        .filter(|v| v.owner == owner)
        .ok_or("write handle unavailable")?;
    write.staging.write_all(&bytes).map_err(|e| e.to_string())
}
pub fn abort(owner: &str, handle: &str) -> Result<(), String> {
    let mut pending = writes().lock().map_err(|_| "write registry unavailable")?;
    if !pending.get(handle).is_some_and(|v| v.owner == owner) {
        return Err("write handle unavailable".into());
    }
    pending.remove(handle);
    Ok(())
}
pub fn commit(owner: &str, handle: &str) -> Result<Value, String> {
    let mut pending = writes().lock().map_err(|_| "write registry unavailable")?;
    let write = pending
        .get_mut(handle)
        .filter(|v| v.owner == owner)
        .ok_or("write handle unavailable")?;
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
    let result = (|| {
        let mut output = target_dir
            .open_with(&temp_name, OpenOptions::new().write(true).create_new(true))
            .map_err(|e| e.to_string())?;
        write
            .staging
            .seek(SeekFrom::Start(0))
            .map_err(|e| e.to_string())?;
        std::io::copy(&mut write.staging, &mut output).map_err(|e| e.to_string())?;
        if write.original.is_some() {
            output
                .set_permissions(
                    dir.metadata(&write.path)
                        .map_err(|e| e.to_string())?
                        .permissions(),
                )
                .map_err(|e| e.to_string())?;
        }
        output.sync_all().map_err(|e| e.to_string())?;
        drop(output);
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
            target_dir
                .rename(&temp_name, &target_dir, name)
                .map_err(|e| e.to_string())?;
        }
        entry(dir, &write.path)
    })();
    if result.is_err() {
        let _ = target_dir.remove_file(&temp_name);
    }
    if result.is_ok() {
        pending.remove(handle);
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
