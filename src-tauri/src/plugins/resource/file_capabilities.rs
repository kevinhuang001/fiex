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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FileCapabilities {
    pub list: bool,
    pub stat: bool,
    pub search: bool,
    pub read: bool,
    pub write: bool,
    pub create: bool,
    pub rename: bool,
    pub remove: bool,
}

impl FileCapabilities {
    pub const ALL: Self = Self {
        list: true,
        stat: true,
        search: true,
        read: true,
        write: true,
        create: true,
        rename: true,
        remove: true,
    };

    pub fn intersect(self, other: Self) -> Self {
        Self {
            list: self.list && other.list,
            stat: self.stat && other.stat,
            search: self.search && other.search,
            read: self.read && other.read,
            write: self.write && other.write,
            create: self.create && other.create,
            rename: self.rename && other.rename,
            remove: self.remove && other.remove,
        }
    }

    /// Each protocol operation has an independent capability.
    pub fn allows(self, operation: FsOperation) -> bool {
        match operation {
            FsOperation::List => self.list,
            FsOperation::Stat => self.stat,
            FsOperation::Search => self.search,
            FsOperation::Read => self.read,
            FsOperation::Write => self.write,
            FsOperation::Mkdir => self.create,
            FsOperation::Rename => self.rename,
            FsOperation::Remove => self.remove,
        }
    }

    pub fn has_mutation_capability(self) -> bool {
        self.write || self.create || self.rename || self.remove
    }

    pub fn into_allowed_ops(self) -> Vec<String> {
        FsOperation::ALL
            .into_iter()
            .filter(|operation| self.allows(*operation))
            .map(|operation| operation.as_str().to_owned())
            .collect()
    }
}

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
