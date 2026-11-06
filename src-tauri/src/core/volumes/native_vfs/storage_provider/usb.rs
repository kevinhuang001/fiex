//! Read-only authorization for volumes opened by the fallback provider.
use super::provider_error;
use crate::usbfs::{self, Route};
use fiex_provider_contracts::{FsOperation, ProviderError};

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
    Err(ProviderError::invalid_params(
        "Unsupported: USB filesystem fallback is read-only",
    ))
}
