//! Temporary read-only exports for OS file drag/open operations. Leases outlive
//! the drag gesture so the receiving application has time to open the files.
use super::*;
use std::{
    collections::HashMap,
    io::Write,
    path::Path,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
const LEASE_TTL: Duration = Duration::from_secs(30 * 60);
const MAX_EXPORT_BYTES: u64 = 100 * 1024 * 1024 * 1024;
const MAX_ITEMS: usize = 100_000;
struct Lease {
    _directory: tempfile::TempDir,
    expires: Instant,
    bytes: u64,
    items: usize,
}

// Windows cannot remove read-only staged files. Release the attribute before
// TempDir performs its cleanup, without following links outside the lease.
#[cfg(windows)]
#[allow(clippy::permissions_set_readonly_false)] // Windows file attribute only; no Unix chmod.
fn unlock_staged(path: &Path, depth: usize) {
    if depth > 128 {
        return;
    }
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return;
    };
    if metadata.file_type().is_symlink() {
        return;
    }
    if metadata.is_dir() {
        if let Ok(children) = std::fs::read_dir(path) {
            for child in children.flatten() {
                unlock_staged(&child.path(), depth + 1);
            }
        }
    } else if metadata.permissions().readonly() {
        let mut permissions = metadata.permissions();
        permissions.set_readonly(false);
        let _ = std::fs::set_permissions(path, permissions);
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        #[cfg(windows)]
        unlock_staged(self._directory.path(), 0);
    }
}
fn leases() -> &'static Mutex<HashMap<String, Lease>> {
    static VALUES: OnceLock<Mutex<HashMap<String, Lease>>> = OnceLock::new();
    let registry = VALUES.get_or_init(Default::default);
    // Leases must expire even if the user never starts another drag.
    // Otherwise the staged data remains on disk until Fiex exits.
    static CLEANUP: OnceLock<()> = OnceLock::new();
    CLEANUP.get_or_init(|| {
        std::thread::spawn(|| loop {
            std::thread::sleep(Duration::from_secs(60));
            prune_leases();
        });
    });
    registry
}

fn prune_leases() {
    let mut expired = Vec::new();
    if let Ok(mut registry) = leases().lock() {
        let now = Instant::now();
        let ids: Vec<_> = registry
            .iter()
            .filter(|(_, lease)| lease.expires <= now)
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            if let Some(lease) = registry.remove(&id) {
                expired.push(lease);
            }
        }
    }
    // Removing temporary directories can block. Never do it while holding
    // the global registry mutex or while another export is registering.
    drop(expired);
}
fn staging_lock() -> &'static Mutex<()> {
    static STAGING: OnceLock<Mutex<()>> = OnceLock::new();
    STAGING.get_or_init(Default::default)
}

pub fn export(paths: &[String]) -> Result<Vec<String>, String> {
    if paths.is_empty() {
        return Err("empty export".into());
    }
    let mut result = Vec::new();
    let mut fresh = Vec::new();
    for path in paths {
        // On macOS/Linux Trash is backed by a real filesystem directory.
        // Never classify it as an ordinary local exportable path.
        if crate::core::volumes::virtual_resolvers::resolver_for(path)
            .is_some_and(|resolver| resolver.id() == "core:trash")
        {
            return Err("Unsupported: Trash items cannot be exported".into());
        }
        // A user-owned application is a virtual source, not a host path.
        // It must pass through the same Broker and export lease as USB/VFS.
        if crate::core::volumes::applications::is_user(path)
            || crate::plugins::mounts::handles(path)
            || crate::usbfs::route(path).is_some()
            || (path.starts_with("recent://") && path != "recent://")
        {
            fresh.push(path.clone());
        } else if crate::core::volumes::virtual_resolvers::opaque(path) {
            return Err("Unsupported: this view cannot export content".into());
        } else {
            let local = Path::new(path);
            if !local.exists() {
                return Err("file not found".into());
            }
            result.push(path.clone());
        }
    }
    if fresh.is_empty() {
        return Ok(result);
    }
    // Serialize staging reservations so concurrent drags cannot transiently
    // fill the disk beyond the quota before either lease is registered.
    let _staging = staging_lock()
        .lock()
        .map_err(|_| "export staging unavailable")?;
    prune_leases();
    let (active_bytes, active_items) = {
        let registry = leases().lock().map_err(|_| "export registry unavailable")?;
        if registry.len() >= 64 {
            return Err("too many active exports".into());
        }
        let used_bytes = registry
            .values()
            .try_fold(0u64, |sum, lease| sum.checked_add(lease.bytes))
            .ok_or("export quota exceeded")?;
        let used_items = registry
            .values()
            .try_fold(0usize, |sum, lease| sum.checked_add(lease.items))
            .ok_or("export item limit exceeded")?;
        (used_bytes, used_items)
    };
    let directory = tempfile::tempdir().map_err(|e| e.to_string())?;
    let mut bytes = active_bytes;
    let mut count = active_items;
    for path in &fresh {
        let broker = super::trusted(path, "core:export")?;
        let info = broker.stat()?;
        let name = info["name"].as_str().ok_or("missing export name")?;
        crate::plugins::mounts::segment(name)?;
        let target = directory.path().join(name);
        if target.exists() {
            return Err("duplicate export name".into());
        }
        export_node(&broker, &target, &mut bytes, &mut count, 0)?;
        result.push(target.to_string_lossy().into_owned());
    }
    // The recursive walker counted active leases in its quota, but this new
    // lease must own only the bytes/items it actually staged.
    let bytes = bytes
        .checked_sub(active_bytes)
        .ok_or("export quota underflow")?;
    let count = count
        .checked_sub(active_items)
        .ok_or("export item underflow")?;
    let id = crate::plugins::mounts::new_id()?;
    let mut registry = leases().lock().map_err(|_| "export registry unavailable")?;
    registry.retain(|_, lease| lease.expires > Instant::now());
    if registry.len() >= 64 {
        return Err("too many active exports".into());
    }
    // Enforce quota across all live leases, not just within one drag. An
    // application may start multiple independent drags before leases expire.
    let active_bytes = registry
        .values()
        .try_fold(0u64, |sum, lease| sum.checked_add(lease.bytes))
        .ok_or("export quota exceeded")?;
    let active_items = registry
        .values()
        .try_fold(0usize, |sum, lease| sum.checked_add(lease.items))
        .ok_or("export item limit exceeded")?;
    if active_bytes
        .checked_add(bytes)
        .is_none_or(|total| total > MAX_EXPORT_BYTES)
        || active_items
            .checked_add(count)
            .is_none_or(|total| total > MAX_ITEMS)
    {
        return Err("active export lease quota exceeded".into());
    }
    registry.insert(
        id,
        Lease {
            _directory: directory,
            expires: Instant::now() + LEASE_TTL,
            bytes,
            items: count,
        },
    );
    Ok(result)
}
fn export_node(
    source: &ResourceBroker,
    target: &Path,
    bytes: &mut u64,
    count: &mut usize,
    depth: usize,
) -> Result<(), String> {
    if depth > 128 || *count >= MAX_ITEMS {
        return Err("export tree limit exceeded".into());
    }
    *count += 1;
    let stat = source.stat()?;
    if stat["symlink"] == true {
        return Err("Unsupported: exporting symlinks requires an explicit policy".into());
    }
    if stat["isDir"] == true {
        source.require(FsOperation::List, false)?;
        std::fs::create_dir(target).map_err(|e| e.to_string())?;
        let listing = source.list()?;
        let children = listing.as_array().ok_or("invalid listing")?;
        for child in children {
            let name = child["name"].as_str().ok_or("missing child name")?;
            crate::plugins::mounts::segment(name)?;
            let path = child["path"].as_str().ok_or("missing child path")?;
            let reference = source.scope.issue_child(
                &serde_json::to_value(&source.reference).map_err(|e| e.to_string())?,
                path,
                None,
            )?;
            let resource =
                ResourceBroker::resolve(&source.scope, &source.owner, &reference["ref"])?;
            export_node(&resource, &target.join(name), bytes, count, depth + 1)?;
        }
        return Ok(());
    }
    // Export is a read operation, not merely a metadata lookup. Check the
    // current node capability before opening the provider stream.
    source.require(FsOperation::Read, false)?;
    let mut read = source.adapter()?.open_read()?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)
        .map_err(|e| e.to_string())?;
    let mut offset = 0u64;
    loop {
        let (part, eof) = read.read_at(offset, CHUNK)?;
        if part.is_empty() {
            if !eof {
                return Err("export stream ended unexpectedly".into());
            }
            break;
        }
        offset = offset
            .checked_add(part.len() as u64)
            .ok_or("export overflow")?;
        *bytes = bytes
            .checked_add(part.len() as u64)
            .filter(|b| *b <= MAX_EXPORT_BYTES)
            .ok_or("export quota exceeded")?;
        file.write_all(&part).map_err(|e| e.to_string())?;
        if eof {
            break;
        }
    }
    if offset != read.size() {
        return Err("incomplete export".into());
    }
    file.sync_all().map_err(|e| e.to_string())?;
    let mut permissions = file.metadata().map_err(|e| e.to_string())?.permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(target, permissions).map_err(|e| e.to_string())?;
    Ok(())
}

// The export manager may stage read-only data on Windows, where DeleteFile
// refuses to remove files carrying FILE_ATTRIBUTE_READONLY.
#[cfg(all(test, windows))]
mod cleanup_tests {
    use super::*;

    #[test]
    fn read_only_staged_files_are_reclaimed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        let child = path.join("payload.txt");
        std::fs::write(&child, "data").unwrap();
        let mut permissions = std::fs::metadata(&child).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&child, permissions).unwrap();
        drop(Lease {
            _directory: dir,
            expires: Instant::now(),
            bytes: 4,
            items: 1,
        });
        assert!(!path.exists());
    }
}
