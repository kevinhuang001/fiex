//! Provider-owned capability checks for USB-backed filesystem resources.
use super::{ensure_access, route, FsKind};

pub(crate) fn writable(path: &str) -> Option<bool> {
    let route = route(path)?;
    Some(
        matches!(route.kind, FsKind::Ntfs | FsKind::Ext4)
            && ensure_access(&route).is_ok()
            && std::fs::OpenOptions::new()
                .write(true)
                .open(&route.driver_device)
                .is_ok(),
    )
}

pub(crate) fn is_root(path: &str) -> Option<bool> {
    route(path).map(|route| route.rel.is_empty())
}
