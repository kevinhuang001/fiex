//! Presentation and dedicated-action policy for the three Core-owned virtual
//! folders (Recent, Applications, Trash). Storage lives in
//! `virtual_storage`; these views never cross plugin IPC.
use super::{applications, recycle, trash};
use crate::core::files::Entry;
use serde_json::json;

pub const RECENT_URI: &str = "recent://";
pub const APPLICATIONS_URI: &str = applications::URI;

#[derive(Clone, Debug)]
pub struct VirtualMenuItem {
    pub id: &'static str,
    pub title: String,
    pub icon: &'static str,
    pub danger: bool,
    pub tail: bool,
}

#[derive(Clone, Debug)]
pub struct VirtualAction {
    pub command: &'static str,
    pub paths: Vec<String>,
}

/// A builtin view contributes menus, icons and dedicated actions. It does not
/// own list/stat/read/remove: those are storage operations on the adapter.
pub trait VirtualView: Send + Sync {
    fn id(&self) -> &'static str;
    fn readonly(&self) -> bool;
    /// Explicit storage-operation ceiling. Presentation and dispatch must not
    /// infer write support from an adapter name or from readonly=false.
    fn operations(&self, path: &str) -> Vec<&'static str>;
    fn menu(&self, path: &str) -> Vec<VirtualMenuItem>;
    fn execute(&self, action: &str, paths: &[String]) -> Result<VirtualAction, String>;
    fn icon(&self, path: &str) -> Option<String>;
    fn default_view(&self) -> Option<&'static str> {
        None
    }
    /// Dedicated launch/restore-style actions are not generic storage IO.
    fn launch(&self, _path: &str) -> Result<(), String> {
        Err("Unsupported: builtin view has no launch action".into())
    }
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
    fn menu(&self, _path: &str) -> Vec<VirtualMenuItem> {
        vec![VirtualMenuItem {
            id: "open",
            title: crate::i18n::t("打开").into(),
            icon: "open",
            danger: false,
            tail: false,
        }]
    }
    fn execute(&self, action: &str, paths: &[String]) -> Result<VirtualAction, String> {
        if action != "open" {
            return Err("unknown recent action".into());
        }
        Ok(VirtualAction {
            command: "open",
            paths: paths
                .iter()
                .map(|path| crate::core::recent::source_for(path))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
    fn icon(&self, _path: &str) -> Option<String> {
        None
    }
    fn launch(&self, path: &str) -> Result<(), String> {
        let original = crate::core::recent::source_for(path)?;
        crate::core::terminals::open_path(&original)
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
    fn menu(&self, path: &str) -> Vec<VirtualMenuItem> {
        let mut items = vec![VirtualMenuItem {
            id: "open",
            title: crate::i18n::t("打开").into(),
            icon: "open",
            danger: false,
            tail: false,
        }];
        if applications::is_user(path) {
            items.push(VirtualMenuItem {
                id: "delete",
                title: crate::i18n::t("彻底删除").into(),
                icon: "trash",
                danger: true,
                tail: true,
            });
        }
        items
    }
    fn execute(&self, action: &str, paths: &[String]) -> Result<VirtualAction, String> {
        match action {
            "open" => Ok(VirtualAction {
                command: "open",
                paths: paths.to_vec(),
            }),
            "delete" => Ok(VirtualAction {
                command: "delete",
                paths: paths.to_vec(),
            }),
            _ => Err("unknown application action".into()),
        }
    }
    fn default_view(&self) -> Option<&'static str> {
        Some("icons")
    }
    fn icon(&self, path: &str) -> Option<String> {
        if applications::is_user(path) {
            return None;
        }
        let entry = applications::list()
            .ok()?
            .into_iter()
            .find(|entry| entry.path == path)?;
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
    fn menu(&self, _path: &str) -> Vec<VirtualMenuItem> {
        vec![
            VirtualMenuItem {
                id: "restore",
                title: crate::i18n::t("放回原处").into(),
                icon: "restore",
                danger: false,
                tail: false,
            },
            VirtualMenuItem {
                id: "delete",
                title: crate::i18n::t("彻底删除").into(),
                icon: "trash",
                danger: true,
                tail: true,
            },
        ]
    }
    fn execute(&self, action: &str, paths: &[String]) -> Result<VirtualAction, String> {
        match action {
            "restore" => Ok(VirtualAction {
                command: "restore",
                paths: paths.to_vec(),
            }),
            "delete" => Ok(VirtualAction {
                command: "delete",
                paths: paths.to_vec(),
            }),
            _ => Err("unknown trash action".into()),
        }
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
    if let Some(root) = trash::trash_path() {
        let root = root.to_string_lossy();
        let root = root.trim_end_matches(['/', '\\']);
        if path == root || path.starts_with(&format!("{root}/")) {
            return Some(&TRASH);
        }
    }
    None
}

pub fn icon_for(path: &str) -> Option<String> {
    view_for(path).and_then(|view| view.icon(path))
}

/// One menu contract for Core-owned and plugin-owned VFS adapters.
pub fn menu_json(view: &dyn VirtualView, paths: &[String]) -> serde_json::Value {
    let items = if paths.is_empty() {
        vec![]
    } else {
        view.menu(&paths[0])
            .into_iter()
            .filter(|item| {
                paths
                    .iter()
                    .all(|path| view.menu(path).iter().any(|other| other.id == item.id))
            })
            .map(|item| {
                json!({
                    "id": item.id, "kind": "action", "title": item.title,
                    "icon": item.icon, "danger": item.danger,
                    "placement": if item.tail { Some("tail") } else { None },
                })
            })
            .collect::<Vec<_>>()
    };
    json!({"items": items})
}

/// URI-backed resources have no host path. Never canonicalize or materialize
/// these identifiers as files, even for metadata-only extension calls.
pub fn opaque(path: &str) -> bool {
    path.starts_with(RECENT_URI) || path.starts_with(APPLICATIONS_URI) || recycle::is_virtual(path)
}

pub fn resource_entry(path: &str) -> Result<Entry, String> {
    super::virtual_storage::entry(path)
}
