use super::{
    plugin,
    routing::{resolve, Route},
    FsError, FsErrorKind, ResourceBroker,
};
use crate::core::volumes::native_vfs;
use fiex_provider_contracts::FsOperation;

pub(in crate::core::fs) fn require(
    resource: &ResourceBroker,
    operation: FsOperation,
    create: bool,
) -> Result<(), FsError> {
    match resolve(&resource.path)? {
        Route::NativeProvider => {
            native_vfs::require_for(resource, operation, create).map_err(FsError::from)
        }
        Route::Plugin => {
            let (mount, node) = if create {
                let (mount, node, _) = plugin::parent(resource)?;
                (mount, node)
            } else {
                plugin::plugin_node(resource)?
            };
            let required = if create && matches!(operation, FsOperation::Write | FsOperation::Mkdir)
            {
                FsOperation::Mkdir
            } else if operation == FsOperation::Search && !mount.supports(FsOperation::Search) {
                FsOperation::List
            } else {
                operation
            };
            if !mount.supports(required) {
                return Err(FsError::new(
                    FsErrorKind::Unsupported,
                    format!("Unsupported: {}", required.as_str()),
                ));
            }
            let entry = mount.stat(&node).map_err(FsError::from)?;
            if !entry.allows(required) {
                return Err(FsError::new(
                    FsErrorKind::Unsupported,
                    format!("Unsupported: node does not permit {}", required.as_str()),
                ));
            }
            Ok(())
        }
    }
}
