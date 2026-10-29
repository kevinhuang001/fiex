//! A device path is not a mount identity: replacement invalidates old refs.
use super::Route;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

fn epochs() -> &'static Mutex<HashMap<String, u64>> {
    static EPOCHS: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();
    EPOCHS.get_or_init(Default::default)
}
pub(super) fn invalidate(device: &str) {
    if let Ok(mut epochs) = epochs().lock() {
        let value = epochs.entry(device.into()).or_default();
        *value = value.wrapping_add(1);
    }
}
pub fn generation(route: &Route) -> Result<String, String> {
    let metadata =
        std::fs::metadata(&route.driver_device).map_err(|_| "StaleRef: USB device disconnected")?;
    let mut digest = Sha256::new();
    digest.update(route.root.as_bytes());
    let epoch = epochs()
        .lock()
        .map_err(|_| "device identity unavailable")?
        .get(&route.device)
        .copied()
        .unwrap_or_default();
    digest.update(epoch.to_le_bytes());
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        digest.update(metadata.dev().to_le_bytes());
        digest.update(metadata.ino().to_le_bytes());
        digest.update(metadata.rdev().to_le_bytes());
    }
    #[cfg(not(unix))]
    {
        digest.update(metadata.len().to_le_bytes());
        digest.update(format!("{:?}", metadata.created().ok()).as_bytes());
    }
    // Linux assigns a new disk sequence on reconnect even if /dev and rdev
    // are reused. Partitions inherit the containing disk's sequence.
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::{FileTypeExt, MetadataExt};
        if metadata.file_type().is_block_device() {
            let sys = std::fs::canonicalize(format!(
                "/sys/dev/block/{}:{}",
                libc::major(metadata.rdev()),
                libc::minor(metadata.rdev())
            ))
            .map_err(|_| "StaleRef: device registration missing")?;
            let sequence = sys
                .ancestors()
                .find_map(|path| std::fs::read(path.join("diskseq")).ok())
                .ok_or("Unsupported: kernel device generation unavailable")?;
            digest.update(sequence);
        }
    }
    Ok(hex::encode(digest.finalize()))
}
