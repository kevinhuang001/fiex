//! Disk backend for visited directory snapshots. No filesystem providers are invoked here.
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

const SCHEMA: u32 = 2;
const MAX_SNAPSHOT_BYTES: usize = 4 * 1024 * 1024;
const MAX_DISK_BYTES: u64 = 64 * 1024 * 1024;
const MAX_FILES: usize = 2_000;
static WRITES: AtomicUsize = AtomicUsize::new(0);

#[derive(Serialize, Deserialize)]
struct Envelope<T> {
    schema: u32,
    key: String,
    value: T,
}

fn root() -> Option<PathBuf> {
    dirs::cache_dir().map(|root| root.join("fiex").join("directory-v2"))
}
fn file(key: &str) -> Option<PathBuf> {
    root().map(|p| {
        p.join(format!(
            "{}.json",
            hex::encode(Sha256::digest(key.as_bytes()))
        ))
    })
}
fn ensure_private(path: &Path) -> std::io::Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}
pub fn load<T: DeserializeOwned>(key: &str) -> Option<T> {
    let path = file(key)?;
    if fs::metadata(&path).ok()?.len() > MAX_SNAPSHOT_BYTES as u64 {
        return None;
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(MAX_SNAPSHOT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    let envelope: Envelope<T> = serde_json::from_slice(&bytes).ok()?;
    (envelope.schema == SCHEMA && envelope.key == key).then_some(envelope.value)
}
pub fn store<T: Serialize>(key: &str, value: &T) {
    let Some(path) = file(key) else { return };
    let Some(root) = path.parent() else { return };
    if ensure_private(root).is_err() {
        return;
    }
    let Ok(data) = serde_json::to_vec(&Envelope {
        schema: SCHEMA,
        key: key.to_owned(),
        value,
    }) else {
        return;
    };
    if data.len() > MAX_SNAPSHOT_BYTES {
        return;
    }
    // A uniquely named temp file avoids a crash leaving a partially written snapshot.
    let Ok(mut temp) = tempfile::Builder::new()
        .prefix(".snapshot-")
        .tempfile_in(root)
    else {
        return;
    };
    if temp.write_all(&data).is_err() || temp.flush().is_err() {
        return;
    }
    // tempfile::persist cannot replace an existing destination on some
    // Windows filesystems. Keep the existing snapshot on unexpected errors,
    // but allow a best-effort remove-and-retry for AlreadyExists.
    match temp.persist(&path) {
        Ok(_) => {}
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            if fs::remove_file(&path).is_ok() {
                let _ = error.file.persist(&path);
            }
        }
        Err(_) => return,
    }
    if WRITES.fetch_add(1, Ordering::Relaxed).is_multiple_of(64) {
        prune(root);
    }
}
pub fn remove(key: &str) {
    if let Some(path) = file(key) {
        let _ = fs::remove_file(path);
    }
}
fn prune(root: &Path) {
    let Ok(items) = fs::read_dir(root) else {
        return;
    };
    let mut files = Vec::new();
    let mut total = 0u64;
    for item in items.flatten() {
        let path = item.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        if let Ok(meta) = item.metadata() {
            total = total.saturating_add(meta.len());
            files.push((path, meta.modified().ok(), meta.len()));
        }
    }
    if total <= MAX_DISK_BYTES && files.len() <= MAX_FILES {
        return;
    }
    files.sort_by_key(|(_, modified, _)| *modified);
    let mut count = files.len();
    for (path, _, size) in files {
        if total <= MAX_DISK_BYTES && count <= MAX_FILES {
            break;
        }
        if fs::remove_file(path).is_ok() {
            total = total.saturating_sub(size);
            count = count.saturating_sub(1);
        }
    }
}
