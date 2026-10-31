//! Presentation and dedicated-action policy for the three Core-owned virtual
//! folders (Recent, Applications, Trash). Storage lives in
//! `virtual_storage`; these views never cross plugin IPC.
use super::{applications, recycle, trash_vfs};
use crate::core::files::Entry;
use crate::plugins::provider_registry::FsOperation;
use crate::plugin_sdk::filesystem::{TransferDirection, TransferPreflight};
use crate::plugins::provider_registry::native::NativeProviderRegistry;
use fiex_provider_contracts::{ProviderKind, ProviderMatchRule};
use std::sync::{Arc, OnceLock};

mod storage;

pub const RECENT_URI: &str = "recent://";
pub const APPLICATIONS_URI: &str = applications::URI;

/// A builtin view owns only virtual storage capabilities, icons and platform
/// launch/reveal operations; menu contributions live in native_views.
pub trait VirtualView: Send + Sync {
    /// Provider-owned URI recognition; never infer that an unknown URI is local.
    fn handles(&self, path: &str) -> bool;
    fn id(&self) -> &'static str;
    fn readonly(&self) -> bool;
    /// Native VFS providers implement the same list/stat operations as
    /// plugin VFS instances; no generic storage layer knows their URI names.
    fn list(&self, path: &str) -> Result<Vec<Entry>, String>;
    fn stat(&self, path: &str) -> Result<Entry, String>;
    /// Explicit storage-operation ceiling. Presentation and dispatch must not
    /// infer write support from an adapter name or from readonly=false.
    fn operations(&self, path: &str) -> Vec<&'static str>;
    /// Enforce the current native instance's policy at invocation time.
    /// ResourceBroker has already authorized the scope; no generic adapter
    /// may identify a builtin view by its URI or provider ID.
    fn require(&self, path: &str, owner: &str, op: FsOperation, create: bool) -> Result<(), String>;
    fn preflight(&self, path: &str, request: &TransferPreflight) -> Result<(), String>;
    fn icon(&self, path: &str) -> Option<String>;
    fn default_view(&self) -> Option<&'static str> {
        None
    }
    /// Dedicated launch/restore-style actions are not generic storage IO.
    fn launch(&self, _path: &str) -> Result<(), String> {
        Err("Unsupported: builtin view has no launch action".into())
    }
    /// Reveal an entry in the platform file manager. Only views that can map a
    /// virtual identifier onto a real host location implement this.
    fn reveal(&self, _path: &str) -> Result<(), String> {
        Err("Unsupported: builtin view has no reveal action".into())
    }
}

/// A Recent alias can be revealed only when its original resource still exists
/// on a local filesystem. Remote, USBFS and plugin-mount sources are excluded:
/// their display paths are not locations a host file manager can select.
pub(crate) fn recent_reveal_source(alias: &str) -> Option<String> {
    let source = crate::core::recent::source_for(alias).ok()?;
    revealable_source(&source).then_some(source)
}

fn revealable_source(source: &str) -> bool {
    let path = std::path::Path::new(source);
    !crate::remote::is_remote(source)
        && !crate::plugins::mounts::handles(source)
        && crate::usbfs::route(source).is_none()
        && !opaque(source)
        && path.is_absolute()
        && path.is_file()
}

struct RecentView;
impl VirtualView for RecentView {
    fn handles(&self, path: &str) -> bool {
        path.starts_with(RECENT_URI)
    }

    fn id(&self) -> &'static str {
        "core:recent"
    }
    fn list(&self, path: &str) -> Result<Vec<Entry>, String> {
        storage::recent_list(path)
    }

    fn stat(&self, path: &str) -> Result<Entry, String> {
        storage::recent_stat(self, path)
    }

    fn readonly(&self) -> bool {
        true
    }
    fn operations(&self, path: &str) -> Vec<&'static str> {
        if path == RECENT_URI {
            // Recent's root is an enumerable directory, not readable content.
            vec!["list", "stat"]
        } else {
            // An alias is a read-only file; its actual source permissions are
            // further narrowed by Recent's entry_for_alias implementation.
            vec!["stat", "read"]
        }
    }
    fn require(&self, path: &str, owner: &str, op: FsOperation, _create: bool) -> Result<(), String> {
        if path == RECENT_URI {
            return if matches!(op, FsOperation::List | FsOperation::Stat | FsOperation::Search) {
                Ok(())
            } else {
                Err("Unsupported: Recent root is read-only".into())
            };
        }
        let source = crate::core::recent::source_for(path)?;
        match op {
            FsOperation::Stat => Ok(()),
            FsOperation::Read => crate::core::fs::trusted(&source, owner)
                .map_err(|error| error.to_string())?
                .require(FsOperation::Read, false)
                .map_err(|error| error.to_string()),
            _ => Err("PermissionDenied: Recent is a read-only view".into()),
        }
    }
    fn preflight(&self, _path: &str, request: &TransferPreflight) -> Result<(), String> {
        if matches!(request.direction, TransferDirection::Import) {
            return Err("PermissionDenied: Recent does not accept imports".into());
        }
        Ok(())
    }
    fn icon(&self, _path: &str) -> Option<String> {
        None
    }
    fn launch(&self, path: &str) -> Result<(), String> {
        let original = crate::core::recent::source_for(path)?;
        crate::core::terminals::open_path(&original)
    }
    fn reveal(&self, path: &str) -> Result<(), String> {
        let source = recent_reveal_source(path)
            .ok_or_else(|| crate::i18n::t("无法在文件管理器中显示该位置").to_string())?;
        crate::core::terminals::reveal(&source)
    }
}

struct ApplicationView;
impl VirtualView for ApplicationView {
    fn handles(&self, path: &str) -> bool {
        path.starts_with(APPLICATIONS_URI)
    }

    fn id(&self) -> &'static str {
        "core:applications"
    }
    fn list(&self, path: &str) -> Result<Vec<Entry>, String> {
        storage::applications_list(path)
    }

    fn stat(&self, path: &str) -> Result<Entry, String> {
        storage::applications_stat(self, path)
    }

    fn readonly(&self) -> bool {
        // The folder itself accepts user shortcuts; each system app entry stays
        // read-only so Core's context-menu resolver can hide delete for it.
        false
    }
    fn operations(&self, path: &str) -> Vec<&'static str> {
        if path == APPLICATIONS_URI {
            vec!["list", "stat", "write", "mkdir"]
        } else if applications::is_user(path) {
            vec!["list", "stat", "read", "write", "mkdir", "rename", "remove"]
        } else {
            vec!["stat"]
        }
    }
    fn require(&self, path: &str, _owner: &str, op: FsOperation, create: bool) -> Result<(), String> {
        if path == APPLICATIONS_URI {
            return if matches!(
                op,
                FsOperation::Stat | FsOperation::List | FsOperation::Mkdir | FsOperation::Search
            ) {
                Ok(())
            } else {
                Err("Unsupported: applications root has no file stream".into())
            };
        }
        if !applications::is_user(path) {
            return if op == FsOperation::Stat {
                Ok(())
            } else {
                Err("Unsupported: system application is launch-only".into())
            };
        }
        let file = applications::candidate_path(path)?;
        let exists = file.exists();
        let directory = file.is_dir();
        if create {
            if !matches!(op, FsOperation::Write | FsOperation::Mkdir) {
                return Err("Unsupported: invalid application create operation".into());
            }
            let parent = file.parent().ok_or("missing application parent")?;
            return if parent.is_dir() && !exists {
                Ok(())
            } else {
                Err("Conflict: application destination is unavailable".into())
            };
        }
        if !exists {
            return Err("NotFound: application item missing".into());
        }
        let allowed = match op {
            FsOperation::Stat => true,
            FsOperation::List | FsOperation::Search | FsOperation::Mkdir => directory,
            FsOperation::Read | FsOperation::Write => !directory,
            FsOperation::Rename | FsOperation::Remove => true,
        };
        if allowed { Ok(()) } else { Err(format!("Unsupported: {}", op.as_str())) }
    }
    fn preflight(&self, _path: &str, request: &TransferPreflight) -> Result<(), String> {
        match request.direction {
            TransferDirection::Export if request.metadata.system && !request.same_mount => Err(
                "PermissionDenied: system applications cannot leave the Applications view".into(),
            ),
            TransferDirection::Import
                if !request.same_mount && request.metadata.executable_shortcut != Some(true) =>
            {
                Err("PermissionDenied: Applications accepts only executable shortcuts".into())
            }
            _ => Ok(()),
        }
    }
    fn default_view(&self) -> Option<&'static str> {
        Some("icons")
    }
    fn icon(&self, path: &str) -> Option<String> {
        if applications::is_user(path) {
            return None;
        }
        // The system-app cache also contains apps mapped into folders; the root
        // listing cache intentionally excludes them.
        let entry = applications::system_entry(path).ok()?;
        applications::cached_icon(entry.link_target.as_deref()?)
    }
    fn launch(&self, path: &str) -> Result<(), String> {
        applications::open(path.strip_prefix(applications::URI).unwrap_or(path))
    }
}

struct TrashView;
impl VirtualView for TrashView {
    fn handles(&self, path: &str) -> bool {
        recycle::is_virtual(path) || trash_vfs::handles(path)
    }

    fn id(&self) -> &'static str {
        "core:trash"
    }
    fn list(&self, path: &str) -> Result<Vec<Entry>, String> {
        storage::trash_list(path)
    }

    fn stat(&self, path: &str) -> Result<Entry, String> {
        storage::trash_stat(self, path)
    }

    fn readonly(&self) -> bool {
        false
    }
    fn operations(&self, _path: &str) -> Vec<&'static str> {
        // Trash is enumerated and inspected through VFS. Restore and purge
        // are dedicated, session-validated menu actions, never generic remove.
        // A specific node's allowed_ops determines whether list is possible.
        vec!["list", "stat"]
    }
    fn require(&self, path: &str, _owner: &str, op: FsOperation, _create: bool) -> Result<(), String> {
        let entry = self.stat(path)?;
        if op == FsOperation::Stat
            || (matches!(op, FsOperation::List | FsOperation::Search)
                && entry.is_dir && !entry.is_symlink)
        {
            Ok(())
        } else {
            Err("PermissionDenied: Trash requires a dedicated Core action".into())
        }
    }
    fn preflight(&self, _path: &str, _request: &TransferPreflight) -> Result<(), String> {
        Err("PermissionDenied: Trash requires dedicated restore/delete actions".into())
    }
    fn icon(&self, _path: &str) -> Option<String> {
        None
    }
    fn launch(&self, _path: &str) -> Result<(), String> {
        Err(crate::i18n::t("回收站项目请使用“放回原处”或“彻底删除”").into())
    }
}

/// Three Core-owned VFS instances are registered through the exact shared
/// ProviderDescriptor/ProviderInstances rules used by SDK plugin providers.
fn registry() -> &'static NativeProviderRegistry<dyn VirtualView> {
    static REGISTERED: OnceLock<NativeProviderRegistry<dyn VirtualView>> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let mut registry: NativeProviderRegistry<dyn VirtualView> =
            NativeProviderRegistry::new(ProviderKind::Vfs);
        let providers: [Arc<dyn VirtualView>; 3] = [
            Arc::new(RecentView),
            Arc::new(ApplicationView),
            Arc::new(TrashView),
        ];
        for provider in providers {
            registry
                .register(provider.id(), 100, ProviderMatchRule::default(), provider)
                .expect("invalid native VFS registration");
        }
        registry
    })
}

pub fn view_for(path: &str) -> Option<&'static dyn VirtualView> {
    registry().find(|provider| provider.handles(path))
}

pub fn icon_for(path: &str) -> Option<String> {
    view_for(path).and_then(|view| view.icon(path))
}

/// URI-backed resources have no host path. Never canonicalize or materialize
/// these identifiers as files, even for metadata-only extension calls.
pub fn opaque(path: &str) -> bool {
    view_for(path).is_some()
}

pub fn resource_entry(path: &str) -> Result<Entry, String> {
    super::virtual_storage::entry(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_vfs_instances_use_shared_contracts_and_claim_only_owned_paths() {
        assert_eq!(registry().len(), 3);
        for descriptor in registry().descriptors() {
            assert_eq!(descriptor.kind, ProviderKind::Vfs);
            descriptor.validate().unwrap();
        }
        assert_eq!(view_for("recent://item").unwrap().id(), "core:recent");
        assert_eq!(
            view_for("applications://item").unwrap().id(),
            "core:applications"
        );
        assert!(view_for("unknown://item").is_none());
        assert!(view_for("fiex://mount/not-a-native-view").is_none());
        assert!(!opaque("unknown://item"));
        assert!(view_for(RECENT_URI).unwrap().stat(RECENT_URI).is_ok());
        assert!(view_for(RECENT_URI).unwrap().list(RECENT_URI).is_ok());
        assert!(view_for(RECENT_URI).unwrap().list("recent://bad").is_err());
    }

    #[test]
    fn source_view_identities_remain_stable() {
        assert_eq!(view_for("recent://").unwrap().id(), "core:recent");
        assert_eq!(
            view_for("applications://").unwrap().id(),
            "core:applications"
        );
        assert_eq!(view_for(trash_vfs::URI).unwrap().id(), "core:trash");
    }

    #[test]
    fn recent_reveal_only_accepts_existing_local_files() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("recent.txt");
        std::fs::write(&file, b"x").unwrap();
        assert!(revealable_source(&file.to_string_lossy()));
        assert!(!revealable_source(&directory.path().to_string_lossy()));
        assert!(!revealable_source("fiex://mount/example/recent.txt"));
        assert!(!revealable_source("recent://invalid"));
    }
}
