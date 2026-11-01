//! Capability-driven file operations.
//!
//! One [`FileCapabilities`] snapshot answers every UI-affordance question and
//! every command guard. It is the intersection of the three layers described by
//! the unified file-system protocol: the provider's declared operations, the
//! node's `allowedOps`/`readonly`, and (implicitly) the fact that Core can only
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

    pub const READ_ONLY: Self = Self {
        list: true,
        stat: true,
        search: true,
        read: true,
        write: false,
        create: false,
        rename: false,
        remove: false,
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

    pub fn is_read_only(self) -> bool {
        !(self.write || self.create || self.rename || self.remove)
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
            if crate::core::volumes::application_policy::protected(path) {
                return FileCapabilities::READ_ONLY;
            }
            FileCapabilities {
                list: entry.is_dir,
                stat: true,
                search: entry.is_dir,
                read: !entry.is_dir,
                write: !entry.is_dir && !entry.readonly,
                create: entry.is_dir && !entry.readonly,
                rename: true,
                // On Unix deletion is governed by the parent directory, not the
                // file's read-only metadata. The OS remains the final authority.
                remove: !cfg!(windows) || !entry.readonly,
            }
        }
        Ok(ResourceOwner::NativeVfs("core:usb-fs")) => {
            let writable = crate::usbfs::writable(path).unwrap_or(false);
            let is_root = crate::usbfs::is_root(path).unwrap_or(true);
            FileCapabilities {
                list: entry.is_dir,
                stat: true,
                search: entry.is_dir,
                read: !entry.is_dir,
                write: writable && !entry.is_dir,
                create: writable && entry.is_dir,
                rename: writable && !is_root,
                remove: writable && !is_root,
            }
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
mod tests {
    use super::*;

    fn entry(path: &str, allowed: &[&str], readonly: bool) -> Entry {
        Entry {
            path: path.into(),
            allowed_ops: allowed.iter().map(|value| value.to_string()).collect(),
            readonly,
            ..Entry::default()
        }
    }

    #[test]
    fn absent_paths_fail_closed() {
        assert_eq!(
            for_path("/fiex-absent-capability-test/example"),
            FileCapabilities::default()
        );
        let local = entry("/tmp/example", &[], true);
        assert_eq!(
            for_selection(&["/tmp/example".into()], &[local]),
            FileCapabilities {
                stat: true,
                read: true,
                rename: true,
                remove: !cfg!(windows),
                ..FileCapabilities::default()
            }
        );
    }

    #[test]
    fn local_node_capabilities_respect_file_vs_directory_kind() {
        let file = entry("/tmp/document.txt", &[], false);
        let file_caps = for_entry(&file.path, &file);
        assert!(file_caps.read && file_caps.write && file_caps.rename);
        assert!(!file_caps.list && !file_caps.search && !file_caps.create);

        let mut folder = entry("/tmp/folder", &[], false);
        folder.is_dir = true;
        let folder_caps = for_entry(&folder.path, &folder);
        assert!(folder_caps.list && folder_caps.search && folder_caps.create);
        assert!(!folder_caps.read && !folder_caps.write);
    }

    #[test]
    fn native_virtual_search_is_advertised_only_for_listable_directories() {
        let recent = for_path("recent://");
        assert!(recent.list && recent.search && recent.stat);
        assert!(!recent.read && !recent.remove && !recent.write);
        let applications = for_path("applications://");
        assert!(applications.list && applications.search && applications.create);
        assert!(!applications.remove);

        let mut folder = entry(
            "trash://Zm9sZGVy",
            &["list", "stat", "search", "remove"],
            false,
        );
        folder.is_dir = true;
        let capabilities = for_entry(&folder.path, &folder);
        assert!(capabilities.list && capabilities.search && capabilities.stat);
        assert!(!capabilities.remove && !capabilities.write);
        let file = entry("trash://ZmlsZQ", &["stat", "search", "remove"], false);
        let capabilities = for_entry(&file.path, &file);
        assert!(capabilities.stat && !capabilities.search && !capabilities.remove);
    }

    #[test]
    fn unsupported_uri_never_inherits_local_capabilities() {
        for path in ["unknown://forged.txt", "unknown:opaque-file"] {
            let forged = entry(path, &["read", "write", "remove"], false);
            assert_eq!(for_entry(path, &forged), FileCapabilities::default());
            assert_eq!(for_path(path), FileCapabilities::default());
        }
    }

    #[test]
    fn unknown_mounts_offer_no_operation() {
        let path = "fiex://mount/test-capability-missing/file.txt";
        assert_eq!(for_path(path), FileCapabilities::default());
        let error = require(path, FsOperation::Remove).unwrap_err();
        assert!(error.contains("remove"), "{error}");
    }

    #[test]
    fn declared_operations_are_narrowed_by_the_node() {
        let declared = [FsOperation::Read, FsOperation::Rename, FsOperation::Remove];
        let narrowed = from_entry(&declared, &entry("/m/a", &["read"], false));
        assert!(narrowed.read);
        assert!(!narrowed.stat && !narrowed.list && !narrowed.search);
        let removable = from_entry(&declared, &entry("/m/a", &["read", "remove"], false));
        assert!(removable.read && removable.remove);
        assert!(!removable.rename && !removable.create);
        // Empty explicitly means no operations, even when the provider is broad.
        let empty = from_entry(&declared, &entry("/m/a", &[], false));
        assert_eq!(empty, FileCapabilities::default());
        // READONLY does not replace per-operation parent/server permissions.
        let readonly = from_entry(&declared, &entry("/m/a", &["remove"], true));
        assert!(readonly.remove && !readonly.read);
    }

    #[test]
    fn trash_does_not_grant_generic_delete() {
        let trashed = entry("recyclebin://an-item", &["stat", "remove"], false);
        let capabilities = for_entry("recyclebin://an-item", &trashed);
        assert!(capabilities.stat);
        assert!(!capabilities.list && !capabilities.read && !capabilities.remove);
        // Nonexistent opaque nodes fail closed, even though their provider is
        // registered and supports stat on actual items.
        assert!(!for_path("recyclebin://an-item").remove);
        assert!(!for_path("recyclebin://an-item").stat);
        assert!(for_path("recyclebin://").list);
        let mut nested = entry("trash://opaque-folder", &["stat", "list", "remove"], false);
        nested.is_dir = true;
        let capabilities = for_entry(&nested.path, &nested);
        assert!(capabilities.stat && capabilities.list);
        assert!(!capabilities.remove && !capabilities.write);
    }

    #[test]
    fn selections_intersect() {
        let empty = FileCapabilities::default();
        assert_eq!(FileCapabilities::ALL.intersect(empty), empty);
        assert_eq!(
            FileCapabilities::ALL.intersect(FileCapabilities::READ_ONLY),
            FileCapabilities::READ_ONLY
        );
        let first = FileCapabilities {
            remove: true,
            ..FileCapabilities::READ_ONLY
        };
        assert!(first
            .intersect(FileCapabilities::READ_ONLY)
            .allows(FsOperation::Read));
        assert!(!first
            .intersect(FileCapabilities::READ_ONLY)
            .allows(FsOperation::Remove));
        // Local paths keep the selection unrestricted when entries are empty.
        assert_eq!(
            for_selection(&["/fiex-absent-capability-test/a".into()], &[]),
            FileCapabilities::default()
        );
        let error = require_selection(
            &["fiex://mount/test-capability-missing/a".into()],
            &[],
            FsOperation::Mkdir,
        )
        .unwrap_err();
        assert!(error.contains("mkdir"), "{error}");
    }
}
