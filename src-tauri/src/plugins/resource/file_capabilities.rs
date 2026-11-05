//! Capability-driven file operations.
//!
//! One [`FileCapabilities`] snapshot answers every UI-affordance question and
//! every command guard. It is the intersection of the three layers described by
//! the unified file-system protocol: the provider's declared operations, the
//! node's `allowedOps`, and (implicitly) the fact that Core can only
//! act through an installed provider. Local file content permissions and USB driver restrictions are reflected
//! here; the OS still enforces ACLs and current access on each actual call.
//!
//! The menu uses [`for_selection`] to decide which items exist; commands use
//! [`require`]/[`require_selection`] so a stale UI can never perform an
//! operation the current provider or node does not support.
use crate::core::files::Entry;
use crate::plugins::provider_registry::FsOperation;
use fiex_resource::FileCapabilities;

/// Intersect a provider declaration with one node's own restrictions.
fn from_entry(declared: &[FsOperation], entry: &Entry) -> FileCapabilities {
    let allows = |operation: FsOperation| {
        declared.contains(&operation)
            && entry
                .allowed_ops
                .iter()
                .any(|value| value == operation.as_str())
    };
    FileCapabilities {
        list: entry.is_dir && allows(FsOperation::List),
        stat: allows(FsOperation::Stat),
        search: entry.is_dir && allows(FsOperation::Search),
        read: allows(FsOperation::Read),
        write: allows(FsOperation::Write),
        create: entry.is_dir && allows(FsOperation::Mkdir),
        rename: allows(FsOperation::Rename),
        trash: allows(FsOperation::Trash),
        remove: allows(FsOperation::Remove),
    }
}

pub(crate) fn for_entry(path: &str, entry: &Entry) -> FileCapabilities {
    use crate::core::fs::provider_registry::ResourceOwner;

    match crate::core::fs::provider_registry::owner(path) {
        Ok(ResourceOwner::LocalFs) => {
            let protected = crate::core::volumes::application_policy::protected(path);
            FileCapabilities {
                list: entry.is_dir,
                stat: true,
                search: entry.is_dir,
                read: !entry.is_dir,
                write: !protected,
                create: entry.is_dir && !protected,
                rename: !protected,
                trash: !protected,
                remove: !protected,
            }
        }
        Ok(ResourceOwner::NativeVfs("core:usb-fs")) => {
            let declared = crate::core::volumes::native_vfs::operations(path);
            from_entry(&declared, entry)
        }
        Ok(ResourceOwner::NativeVfs(_)) => {
            let declared = crate::core::volumes::native_vfs::operations(path);
            from_entry(&declared, entry)
        }
        Ok(ResourceOwner::PluginMount {
            plugin_id,
            mount_id,
        }) => crate::plugins::provider_registry::operations_mount(&plugin_id, &mount_id)
            .map(|declared| from_entry(&declared, entry))
            .unwrap_or_default(),
        Err(_) => FileCapabilities::default(),
    }
}

/// Capabilities of a single path. Local paths are unrestricted; a mount is the
/// provider's declared operations narrowed by the node's entry.
pub fn for_path(path: &str) -> FileCapabilities {
    use crate::core::fs::provider_registry::ResourceOwner;

    match crate::core::fs::provider_registry::owner(path) {
        Ok(ResourceOwner::LocalFs) => crate::core::files::entry(std::path::Path::new(path))
            .map(|entry| for_entry(path, &entry))
            .unwrap_or_default(),
        Ok(_) => crate::core::fs::provider_registry::resource_entry(path)
            .map(|entry| for_entry(path, &entry))
            .unwrap_or_default(),
        Err(_) => FileCapabilities::default(),
    }
}

/// Capabilities common to a selection. Local entries and paths are unrestricted.
pub fn for_selection(paths: &[String], entries: &[Entry]) -> FileCapabilities {
    let mut capabilities = FileCapabilities::ALL;
    for (index, path) in paths.iter().enumerate() {
        let next = match entries.get(index) {
            Some(entry) => for_entry(path, entry),
            None => for_path(path),
        };
        capabilities = capabilities.intersect(next);
    }
    capabilities
}

fn missing(operation: FsOperation) -> String {
    format!(
        "{} ({})",
        crate::i18n::t("此位置不支持该操作"),
        operation.as_str()
    )
}

/// Guards a command. Mount operations must be re-checked because the menu is a
/// snapshot; local paths fall through to the OS.
pub fn require(path: &str, operation: FsOperation) -> Result<(), String> {
    if for_path(path).allows(operation) {
        Ok(())
    } else {
        Err(missing(operation))
    }
}

/// Guard for a multi-path selection, using entries the UI already resolved.
pub fn require_selection(
    paths: &[String],
    entries: &[Entry],
    operation: FsOperation,
) -> Result<(), String> {
    if for_selection(paths, entries).allows(operation) {
        Ok(())
    } else {
        Err(missing(operation))
    }
}

#[cfg(test)]
mod tests;
