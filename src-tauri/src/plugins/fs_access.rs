//! File information and shared-buffer access for plugin host methods.
//!
//! This is a first implementation of the generic file SDK. It is scoped to the
//! active explorer directory and uses the legacy filesystem grants. The
//! folder/file split and interactive leases are layered on top of this module.
use crate::plugin_ipc;
use serde_json::{json, Value};
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

const MAX_ENTRIES: usize = 5000;
const MAX_READ: usize = 32 * 1024 * 1024;

fn active() -> Result<PathBuf, String> {
    crate::plugin_host::active_path().ok_or_else(|| "no active directory".to_string())
}

fn resolve(relative: &str) -> Result<PathBuf, String> {
    let base = active()?;
    let base = dunce::canonicalize(&base).map_err(|error| error.to_string())?;
    let relative = relative.trim();
    let relative = if relative.is_empty() { "." } else { relative };
    let requested = Path::new(relative);
    if requested.is_absolute()
        || requested
            .components()
            .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
    {
        return Err("path must stay inside the active directory".into());
    }
    let candidate = base.join(requested);
    if candidate.exists() {
        let real = dunce::canonicalize(&candidate).map_err(|error| error.to_string())?;
        if !real.starts_with(&base) {
            return Err("path escapes the active directory".into());
        }
        return Ok(real);
    }
    let parent = candidate.parent().ok_or("invalid path")?;
    let parent = dunce::canonicalize(parent).map_err(|error| error.to_string())?;
    if !parent.starts_with(&base) {
        return Err("path escapes the active directory".into());
    }
    let name = candidate.file_name().ok_or("invalid path")?;
    Ok(parent.join(name))
}

fn entry_value(path: &Path, relative: &str) -> Result<Value, String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    let is_symlink = metadata.file_type().is_symlink();
    let is_dir = metadata.is_dir();
    let name = path
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| relative.to_owned());
    let extension = if is_dir {
        String::new()
    } else {
        path.extension()
            .map(|value| value.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default()
    };
    Ok(json!({
        "path": relative.replace('\\', "/"),
        "name": name,
        "isDir": is_dir,
        "size": if is_dir { 0 } else { metadata.len() },
        "modified": metadata.modified().ok()
            .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|value| value.as_secs()).unwrap_or(0),
        "extension": extension,
        "mime": if is_dir { "inode/directory".to_string() } else { mime_guess::from_path(path).first_or_octet_stream().to_string() },
        "readonly": metadata.permissions().readonly(),
        "symlink": is_symlink
    }))
}

pub fn list(relative: &str) -> Result<Value, String> {
    let directory = resolve(relative)?;
    if !directory.is_dir() {
        return Err("not a directory".into());
    }
    let mut entries = Vec::new();
    for item in fs::read_dir(&directory).map_err(|error| error.to_string())? {
        if entries.len() >= MAX_ENTRIES {
            break;
        }
        let item = item.map_err(|error| error.to_string())?;
        let path = item.path();
        let name = item.file_name().to_string_lossy().into_owned();
        let child = if relative.trim().is_empty() || relative == "." {
            name
        } else {
            format!("{}/{}", relative.trim_end_matches('/'), name)
        };
        entries.push(entry_value(&path, &child)?);
    }
    entries.sort_by(|a, b| {
        a.get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_lowercase()
            .cmp(
                &b.get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_lowercase(),
            )
    });
    Ok(json!(entries))
}

pub fn stat(relative: &str) -> Result<Value, String> {
    let path = resolve(relative)?;
    entry_value(&path, relative)
}

pub fn create(relative: &str) -> Result<Value, String> {
    let path = resolve(relative)?;
    if path.exists() {
        return Err("file already exists".into());
    }
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| error.to_string())?;
    stat(relative)
}

pub fn read(owner: &str, relative: &str, offset: u64, length: usize) -> Result<Value, String> {
    let path = resolve(relative)?;
    let mut file = fs::File::open(&path).map_err(|error| error.to_string())?;
    let total = file.metadata().map_err(|error| error.to_string())?.len();
    let length = length.min(MAX_READ);
    file.seek(SeekFrom::Start(offset))
        .map_err(|error| error.to_string())?;
    let mut bytes = vec![0u8; length];
    let read = file.read(&mut bytes).map_err(|error| error.to_string())?;
    bytes.truncate(read);
    let descriptor = plugin_ipc::create(owner, read.max(1), false)?;
    plugin_ipc::write(&descriptor.id, &bytes)?;
    Ok(json!({
        "buffer": descriptor,
        "size": read,
        "eof": offset + read as u64 >= total
    }))
}

pub fn write(owner: &str, relative: &str, buffer: &str, mode: &str) -> Result<Value, String> {
    let path = resolve(relative)?;
    let exists = path.exists();
    if mode == "create" && exists {
        return Err("file already exists".into());
    }
    if mode != "create" && !exists {
        return Err("file does not exist".into());
    }
    let bytes = plugin_ipc::read(owner, buffer)?;
    let parent = path.parent().ok_or("invalid path")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temporary = parent.join(format!(
        ".fiex-write-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or_default()
    ));
    {
        let mut file = fs::File::create(&temporary).map_err(|error| error.to_string())?;
        file.write_all(&bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
    }
    if exists {
        fs::remove_file(&path).map_err(|error| error.to_string())?;
    }
    fs::rename(&temporary, &path).map_err(|error| error.to_string())?;
    stat(relative)
}

pub fn remove(relative: &str) -> Result<(), String> {
    let path = resolve(relative)?;
    if path.is_dir() {
        fs::remove_dir_all(&path).map_err(|error| error.to_string())
    } else {
        fs::remove_file(&path).map_err(|error| error.to_string())
    }
}

pub fn rename(from: &str, to: &str) -> Result<(), String> {
    let from = resolve(from)?;
    let to = resolve(to)?;
    fs::rename(from, to).map_err(|error| error.to_string())
}
