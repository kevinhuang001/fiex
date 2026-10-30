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

/// What a mount's provider declared. `None` means "not a mount" only for local
/// paths; callers distinguish the two cases before asking.
fn declared_operations(path: &str) -> Option<Vec<FsOperation>> {
    crate::plugins::resolvers::fs::mount_plugin_id(path)
        .ok()
        .and_then(|plugin_id| crate::plugins::provider_registry::operations(&plugin_id))
}

/// One node entry resolved through the right adapter (connection or view).
fn node_entry(path: &str) -> Result<Entry, String> {
    if crate::remote::is_remote(path) {
        crate::remote::stat(path)
    } else {
        crate::plugins::vfs::stat(path)
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
        list: allows(FsOperation::List),
        stat: allows(FsOperation::Stat),
        search: allows(FsOperation::Search),
        read: allows(FsOperation::Read),
        write: allows(FsOperation::Write),
        create: entry.is_dir && allows(FsOperation::Mkdir),
        rename: allows(FsOperation::Rename),
        remove: allows(FsOperation::Remove),
    }
}

pub(crate) fn for_entry(path: &str, entry: &Entry) -> FileCapabilities {
    if crate::core::volumes::virtual_resolvers::view_for(path)
        .is_some_and(|p| p.id() == "core:trash")
    {
        // Trash supports metadata and root enumeration, not generic
        // content reads, mutations or permanent deletion.
        return FileCapabilities {
            stat: true,
            list: path == crate::core::volumes::recycle::URI
                || crate::core::volumes::trash_path()
                    .is_some_and(|root| root.to_string_lossy() == path),
            ..FileCapabilities::default()
        };
    }
    if crate::core::volumes::virtual_resolvers::opaque(path) {
        let declared = crate::core::volumes::virtual_resolvers::view_for(path)
            .map(|provider| {
                provider
                    .operations(path)
                    .into_iter()
                    .filter_map(FsOperation::parse)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        return from_entry(&declared, entry);
    }
    if let Some(route) = crate::usbfs::route(path) {
        let writable = matches!(
            route.kind,
            crate::usbfs::FsKind::Ntfs | crate::usbfs::FsKind::Ext4
        ) && crate::usbfs::ensure_access(&route).is_ok()
            && std::fs::OpenOptions::new()
                .write(true)
                .open(&route.driver_device)
                .is_ok();
        return FileCapabilities {
            list: entry.is_dir,
            stat: true,
            search: entry.is_dir,
            read: !entry.is_dir,
            write: writable && !entry.is_dir,
            create: writable && entry.is_dir,
            rename: writable && !route.rel.is_empty(),
            remove: writable && !route.rel.is_empty(),
        };
    }
    if crate::core::volumes::application_policy::protected(path) {
        return FileCapabilities::READ_ONLY;
    }
    if !crate::plugins::mounts::handles(path) {
        return FileCapabilities {
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
        };
    }
    declared_operations(path)
        .map(|declared| from_entry(&declared, entry))
        .unwrap_or_default()
}

/// Capabilities of a single path. Local paths are unrestricted; a mount is the
/// provider's declared operations narrowed by the node's entry.
pub fn for_path(path: &str) -> FileCapabilities {
    if crate::core::volumes::virtual_resolvers::view_for(path)
        .is_some_and(|p| p.id() == "core:trash")
    {
        // Trash supports metadata and root enumeration, not generic
        // content reads, mutations or permanent deletion.
        return FileCapabilities {
            stat: true,
            list: path == crate::core::volumes::recycle::URI
                || crate::core::volumes::trash_path()
                    .is_some_and(|root| root.to_string_lossy() == path),
            ..FileCapabilities::default()
        };
    }
    if crate::core::volumes::virtual_resolvers::opaque(path) {
        return crate::core::volumes::virtual_resolvers::resource_entry(path)
            .map(|entry| for_entry(path, &entry))
            .unwrap_or_default();
    }
    if let Some(route) = crate::usbfs::route(path) {
        return crate::usbfs::entry(&route)
            .map(|entry| for_entry(path, &entry))
            .unwrap_or_default();
    }
    if !crate::plugins::mounts::handles(path) {
        return crate::core::files::entry(std::path::Path::new(path))
            .map(|entry| for_entry(path, &entry))
            .unwrap_or_default();
    }
    let Some(declared) = declared_operations(path) else {
        return FileCapabilities::default();
    };
    match node_entry(path) {
        Ok(entry) => from_entry(&declared, &entry),
        // An unreachable or unknown node exposes nothing; the menu omits
        // mutating items rather than offering an operation Core cannot verify.
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
        let trashed = entry("recyclebin://an-item", &["remove"], false);
        assert!(!for_entry("recyclebin://an-item", &trashed).remove);
        assert!(!for_path("recyclebin://an-item").remove);
        assert!(for_path("recyclebin://an-item").stat);
        assert!(for_path("recyclebin://").list);
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
