use super::*;
/// Read-only exports for external applications. No tree snapshots or persistent
/// blob cache: temp directories live until process exit, independently of tabs,
/// so closing one tab cannot invalidate another consumer's export.
pub(super) fn exports(
) -> &'static std::sync::Mutex<std::collections::HashMap<String, tempfile::TempDir>> {
    static EXPORTS: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, tempfile::TempDir>>,
    > = std::sync::OnceLock::new();
    EXPORTS.get_or_init(Default::default)
}

// On Windows, writable temp exports must be cleared before removal. This code
// is compiled only for Windows; its Unix permission semantics are irrelevant.
#[cfg_attr(windows, allow(clippy::permissions_set_readonly_false))]
pub fn cleanup_exports() {
    if let Ok(mut exports) = exports().lock() {
        #[cfg(windows)]
        for directory in exports.values() {
            if let Ok(entries) = std::fs::read_dir(directory.path()) {
                for entry in entries.flatten() {
                    if let Ok(metadata) = entry.metadata() {
                        let mut permissions = metadata.permissions();
                        permissions.set_readonly(false);
                        let _ = std::fs::set_permissions(entry.path(), permissions);
                    }
                }
            }
        }
        exports.clear();
    }
}

pub fn materialize(path: &str) -> Result<String, String> {
    let entry = stat(path)?; // rechecks the owner and scope even on export reuse
    if entry.is_dir {
        return Err("VFS directories cannot be opened as OS folders".into());
    }
    crate::plugins::mounts::segment(&entry.name)?;
    let mut exports = exports()
        .lock()
        .map_err(|_| "export registry unavailable")?;
    if let Some(directory) = exports.get(path) {
        return Ok(directory
            .path()
            .join(&entry.name)
            .to_string_lossy()
            .into_owned());
    }
    if exports.len() >= 64 {
        return Err("too many open VFS exports; restart Fiex to release temporary exports".into());
    }
    let (uri, view, node_id) = node(path)?;
    let mount = mount_for(&uri, &view);
    let stream = mount.open_read(&node_id)?;
    let directory = tempfile::tempdir().map_err(|e| e.to_string())?;
    let target = directory.path().join(&entry.name);
    let mut written = 0u64;
    {
        use std::io::Write;
        let mut file = std::fs::File::create(&target).map_err(|e| e.to_string())?;
        let mut offset = 0u64;
        loop {
            let (chunk, eof) = stream.read_at(offset, READ_CHUNK)?;
            if chunk.is_empty() {
                break;
            }
            file.write_all(&chunk).map_err(|e| e.to_string())?;
            offset += chunk.len() as u64;
            written += chunk.len() as u64;
            if eof {
                break;
            }
        }
        file.flush().map_err(|e| e.to_string())?;
    }
    if stream.size().is_some_and(|expected| expected != written) {
        return Err("VFS export is incomplete".into());
    }
    let mut permissions = std::fs::metadata(&target)
        .map_err(|e| e.to_string())?
        .permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&target, permissions).map_err(|e| e.to_string())?;
    let result = target.to_string_lossy().into_owned();
    exports.insert(path.into(), directory);
    Ok(result)
}
