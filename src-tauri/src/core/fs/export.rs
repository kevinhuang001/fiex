//! Temporary read-only exports for OS file drag/open operations. Leases outlive
//! the drag gesture so the receiving application has time to open the files.
use super::*;
use std::{
    io::Write,
    path::Path,
    time::{Duration, Instant},
};
const LEASE_TTL: Duration = Duration::from_secs(30 * 60);
const MAX_EXPORT_BYTES: u64 = 100 * 1024 * 1024 * 1024;
const MAX_ITEMS: usize = 100_000;
mod lease;
use lease::{leases, prune_leases, root_dir, staging_lock, Lease};

pub fn initialize() {
    prune_leases();
}

pub fn export(paths: &[String]) -> Result<Vec<String>, String> {
    if paths.is_empty() {
        return Err("empty export".into());
    }
    if paths.len() > MAX_ITEMS {
        return Err("export item limit exceeded".into());
    }
    let mut result = vec![String::new(); paths.len()];
    let mut fresh = Vec::new();
    for (index, path) in paths.iter().enumerate() {
        let resource = super::trusted(path, "core:export")?;
        let info = resource.stat()?;
        if info["symlink"] == true {
            return Err("Unsupported: exporting symlinks requires an explicit policy".into());
        }
        resource.require(
            if info["isDir"] == true {
                FsOperation::List
            } else {
                FsOperation::Read
            },
            false,
        )?;
        let native = resource.adapter()?.native_export_path();
        if let Some(local) = native {
            result[index] = local.to_string_lossy().into_owned();
        } else {
            fresh.push((index, resource));
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
    let directory = tempfile::Builder::new()
        .prefix("lease-")
        .tempdir_in(root_dir()?)
        .map_err(|e| e.to_string())?;
    let lease_lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(directory.path().join(".lease-lock"))
        .map_err(|e| e.to_string())?;
    lease_lock.try_lock().map_err(|e| e.to_string())?;
    let mut bytes = active_bytes;
    let mut count = active_items;
    for (index, broker) in &fresh {
        let info = broker.stat()?;
        let name = info["name"].as_str().ok_or("missing export name")?;
        crate::plugins::mounts::segment(name)?;
        // Independent selections may legitimately have identical names.
        // Isolate their staging roots while preserving each exported basename.
        let root = directory.path().join(index.to_string());
        std::fs::create_dir(&root).map_err(|e| e.to_string())?;
        let target = root.join(name);
        export_node(broker, &target, &mut bytes, &mut count, 0)?;
        result[*index] = target.to_string_lossy().into_owned();
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
            lock: Some(lease_lock),
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
