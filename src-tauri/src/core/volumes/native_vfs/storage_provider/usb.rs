//! USB-specific authorization and recursive removal policy.
use super::provider_error;
use crate::{
    plugins::provider_registry::FsOperation,
    usbfs::{self, Route},
};
use fiex_provider_contracts::ProviderError;

pub(super) fn authorize(
    route: &Route,
    operation: FsOperation,
    create: bool,
) -> Result<(), ProviderError> {
    usbfs::ensure_access(route).map_err(provider_error)?;
    if !create {
        let entry = usbfs::entry(route).map_err(provider_error)?;
        if (operation == FsOperation::Read && entry.is_dir)
            || (matches!(
                operation,
                FsOperation::List | FsOperation::Search | FsOperation::Mkdir
            ) && !entry.is_dir)
        {
            return Err(ProviderError::invalid_params(
                "Unsupported: operation does not apply to node kind",
            ));
        }
    }
    if matches!(
        operation,
        FsOperation::List | FsOperation::Stat | FsOperation::Read | FsOperation::Search
    ) {
        return Ok(());
    }
    if matches!(
        route.kind,
        usbfs::FsKind::Apfs | usbfs::FsKind::Hfs | usbfs::FsKind::Btrfs
    ) {
        return Err(ProviderError::invalid_params(
            "Unsupported: read-only USB filesystem",
        ));
    }
    std::fs::OpenOptions::new()
        .write(true)
        .open(&route.driver_device)
        .map_err(|_| {
            ProviderError::invalid_params("PermissionDenied: OS has not granted raw-device writes")
        })?;
    if route.rel.is_empty()
        && matches!(
            operation,
            FsOperation::Remove | FsOperation::Rename | FsOperation::Write
        )
    {
        return Err(ProviderError::invalid_params("cannot modify USB root"));
    }
    Ok(())
}

pub(super) fn remove_tree(route: &Route, depth: usize) -> Result<(), String> {
    if depth > 128 {
        return Err("USB tree limit exceeded".into());
    }
    if usbfs::entry(route)?.is_dir {
        for child in usbfs::listing(route)? {
            let next = usbfs::route(&child.path).ok_or("invalid USB child")?;
            if next.root != route.root {
                return Err("USB child escaped mount".into());
            }
            remove_tree(&next, depth + 1)?;
        }
    }
    usbfs::remove(route)
}
