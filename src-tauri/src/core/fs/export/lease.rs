use super::*;
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

pub(super) struct Lease {
    pub(super) _directory: tempfile::TempDir,
    pub(super) lock: Option<std::fs::File>,
    pub(super) expires: Instant,
    pub(super) bytes: u64,
    pub(super) items: usize,
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
        self.lock.take();
        #[cfg(windows)]
        unlock_staged(self._directory.path(), 0);
    }
}
pub(super) fn leases() -> &'static Mutex<HashMap<String, Lease>> {
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

pub(super) fn prune_leases() {
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
    // Staging directories survive a crash so that another process can finish
    // opening the exported files. Reclaim orphaned leases after the same TTL
    // on a later start; never race an export still being prepared.
    let Ok(_staging) = staging_lock().try_lock() else {
        return;
    };
    let Ok(root) = root_dir() else {
        return;
    };
    let live = leases()
        .lock()
        .ok()
        .map(|registry| {
            registry
                .values()
                .map(|lease| lease._directory.path().to_path_buf())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            if !entry.file_name().to_string_lossy().starts_with("lease-")
                || !entry.file_type().is_ok_and(|kind| kind.is_dir())
                || live.contains(&entry.path())
            {
                continue;
            }
            let expired = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|time| time.elapsed().ok())
                .is_some_and(|age| age > LEASE_TTL);
            if expired {
                let lock_path = entry.path().join(".lease-lock");
                let lock = if lock_path.exists() {
                    if std::fs::symlink_metadata(&lock_path)
                        .is_ok_and(|m| m.file_type().is_symlink())
                    {
                        continue;
                    }
                    let Ok(file) = std::fs::OpenOptions::new()
                        .read(true)
                        .write(true)
                        .open(lock_path)
                    else {
                        continue;
                    };
                    if file.try_lock().is_err() {
                        continue;
                    }
                    Some(file)
                } else {
                    None
                };
                // Another Fiex process may still own this lease. The private
                // lock prevents orphan cleanup from touching its live export.
                drop(lock);
                #[cfg(windows)]
                unlock_staged(&entry.path(), 0);
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }
}
pub(super) fn staging_lock() -> &'static Mutex<()> {
    static STAGING: OnceLock<Mutex<()>> = OnceLock::new();
    STAGING.get_or_init(Default::default)
}

pub(super) fn root_dir() -> Result<std::path::PathBuf, String> {
    let root = dirs::cache_dir()
        .ok_or("export cache directory unavailable")?
        .join("fiex")
        .join("exports");
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    Ok(root)
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
            lock: None,
            expires: Instant::now(),
            bytes: 4,
            items: 1,
        });
        assert!(!path.exists());
    }
}
