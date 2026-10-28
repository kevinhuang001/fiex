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
        // Local files already have a native path. A view of a user-owned app
        // resolves through its confined store before reaching this boundary.
        if crate::core::volumes::applications::is_user(path) {
            result.push(
                crate::core::volumes::applications::stored_path(path)?
                    .to_string_lossy()
                    .into_owned(),
            );
        } else if crate::plugins::mounts::handles(path) || crate::usbfs::route(path).is_some() {
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
    let directory = tempfile::tempdir().map_err(|e| e.to_string())?;
    let mut bytes = 0u64;
    let mut count = 0usize;
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
