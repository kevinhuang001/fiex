//! Presentation and dedicated-action policy for the three Core-owned virtual
//! folders (Recent, Applications, Trash). Storage lives in
//! `virtual_storage`; these views never cross plugin IPC.
use super::{applications, recycle, trash_vfs};
use crate::core::files::Entry;

pub const RECENT_URI: &str = "recent://";
pub const APPLICATIONS_URI: &str = applications::URI;

/// A builtin view owns only virtual storage capabilities, icons and platform
/// launch/reveal operations; menu contributions live in native_views.
pub trait VirtualView: Send + Sync {
    fn id(&self) -> &'static str;
    fn readonly(&self) -> bool;
    /// Explicit storage-operation ceiling. Presentation and dispatch must not
    /// infer write support from an adapter name or from readonly=false.
    fn operations(&self, path: &str) -> Vec<&'static str>;
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
    fn id(&self) -> &'static str {
        "core:recent"
    }
    fn readonly(&self) -> bool {
        true
    }
    fn operations(&self, _path: &str) -> Vec<&'static str> {
        vec!["list", "stat", "read"]
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
    fn id(&self) -> &'static str {
        "core:applications"
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
    fn id(&self) -> &'static str {
        "core:trash"
    }
    fn readonly(&self) -> bool {
        false
    }
    fn operations(&self, _path: &str) -> Vec<&'static str> {
        vec!["list", "stat", "remove"]
    }
    fn icon(&self, _path: &str) -> Option<String> {
        None
    }
    fn launch(&self, _path: &str) -> Result<(), String> {
        Err(crate::i18n::t("回收站项目请使用“放回原处”或“彻底删除”").into())
    }
}

static RECENT: RecentView = RecentView;
static APPLICATIONS: ApplicationView = ApplicationView;
static TRASH: TrashView = TrashView;

pub fn view_for(path: &str) -> Option<&'static dyn VirtualView> {
    if path.starts_with(RECENT_URI) {
        return Some(&RECENT);
    }
    if path == APPLICATIONS_URI || path.starts_with(applications::URI) {
        return Some(&APPLICATIONS);
    }
    if recycle::is_virtual(path) {
        return Some(&TRASH);
    }
    if trash_vfs::handles(path) {
        return Some(&TRASH);
    }
    None
}

pub fn icon_for(path: &str) -> Option<String> {
    view_for(path).and_then(|view| view.icon(path))
}

/// URI-backed resources have no host path. Never canonicalize or materialize
/// these identifiers as files, even for metadata-only extension calls.
pub fn opaque(path: &str) -> bool {
    path.starts_with(RECENT_URI)
        || path.starts_with(APPLICATIONS_URI)
        || recycle::is_virtual(path)
        || trash_vfs::handles(path)
}

pub fn resource_entry(path: &str) -> Result<Entry, String> {
    super::virtual_storage::entry(path)
}

#[cfg(test)]
mod tests {
    use super::*;

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
