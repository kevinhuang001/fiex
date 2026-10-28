//! The three Core-owned virtual folders (Recent, Applications, Trash) share
//! one provider surface. They never cross plugin IPC: Core lists, opens,
//! supplies data locally; all presentation contributions use EventRouter.

use super::{applications, recycle, trash};
#[cfg(not(target_os = "windows"))]
use crate::core::files;
use crate::core::files::Entry;
use serde_json::json;
#[cfg(not(target_os = "windows"))]
use std::path::Path;

pub const RECENT_URI: &str = "recent://";
pub const APPLICATIONS_URI: &str = applications::URI;

pub struct VirtualContext<'a> {
    pub recent: &'a [Entry],
}

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

pub trait VirtualResolver: Send + Sync {
    fn id(&self) -> &'static str;
    fn readonly(&self) -> bool;
    /// Explicit storage-operation ceiling. Presentation and dispatch must not
    /// infer write support from an adapter name or from readonly=false.
    fn operations(&self, path: &str) -> Vec<&'static str> {
        match self.id() {
            "core:recent" => vec!["list", "stat", "read"],
            "core:trash" => vec!["list", "stat", "remove"],
            "core:applications" if path == APPLICATIONS_URI => {
                vec!["list", "stat", "write", "mkdir"]
            }
            "core:applications" if applications::is_user(path) => {
                vec!["list", "stat", "read", "write", "mkdir", "rename", "remove"]
            }
            _ => vec!["stat"],
        }
    }
    fn list(&self, path: &str, ctx: &VirtualContext<'_>) -> Result<Vec<Entry>, String>;
    fn stat(&self, path: &str, ctx: &VirtualContext<'_>) -> Result<Entry, String>;
    fn open(&self, path: &str) -> Result<(), String>;
    fn remove(&self, paths: &[String]) -> Result<usize, String>;
    fn menu(&self, path: &str) -> Vec<VirtualMenuItem>;
    fn execute(&self, action: &str, paths: &[String]) -> Result<VirtualAction, String>;
    fn icon(&self, path: &str) -> Option<String>;
    fn default_view(&self) -> Option<&'static str> {
        None
    }
}

struct RecentResolver;
impl VirtualResolver for RecentResolver {
    fn id(&self) -> &'static str {
        "core:recent"
    }
    fn readonly(&self) -> bool {
        true
    }
    fn list(&self, path: &str, ctx: &VirtualContext<'_>) -> Result<Vec<Entry>, String> {
        let _ = path;
        Ok(ctx.recent.to_vec())
    }
    fn stat(&self, path: &str, _ctx: &VirtualContext<'_>) -> Result<Entry, String> {
        crate::core::recent::entry_for_alias(path)
    }
    fn open(&self, path: &str) -> Result<(), String> {
        let original = crate::core::recent::source_for(path)?;
        crate::core::terminals::open_path(&original)
    }
    fn remove(&self, _paths: &[String]) -> Result<usize, String> {
        Err(crate::i18n::t("最近使用项目不能在 Fiex 中删除").into())
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
}

struct ApplicationResolver;
impl VirtualResolver for ApplicationResolver {
    fn id(&self) -> &'static str {
        "core:applications"
    }
    fn readonly(&self) -> bool {
        // The folder itself accepts user shortcuts; each system app entry stays
        // read-only so Core's context-menu resolver can hide delete for it.
        false
    }
    fn list(&self, path: &str, _ctx: &VirtualContext<'_>) -> Result<Vec<Entry>, String> {
        applications::list_at(path)
    }
    fn stat(&self, path: &str, _ctx: &VirtualContext<'_>) -> Result<Entry, String> {
        if applications::is_user(path) {
            return applications::stored_entry(path);
        }
        applications::list()?
            .into_iter()
            .find(|entry| entry.path == path)
            .ok_or_else(|| crate::i18n::t("应用不存在").into())
    }
    fn open(&self, path: &str) -> Result<(), String> {
        applications::open(path.strip_prefix(applications::URI).unwrap_or(path))
    }
    fn remove(&self, paths: &[String]) -> Result<usize, String> {
        let mut removed = 0;
        for path in paths {
            applications::remove_user(path)?;
            removed += 1;
        }
        Ok(removed)
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
}

struct TrashResolver;
impl VirtualResolver for TrashResolver {
    fn id(&self) -> &'static str {
        "core:trash"
    }
    fn readonly(&self) -> bool {
        false
    }
    fn list(&self, path: &str, _ctx: &VirtualContext<'_>) -> Result<Vec<Entry>, String> {
        let _ = path;
        #[cfg(target_os = "windows")]
        {
            recycle::list()
        }
        #[cfg(not(target_os = "windows"))]
        {
            let path = trash::trash_path().ok_or(crate::i18n::t("无法定位废纸篓"))?;
            files::list(&path.to_string_lossy(), true, &[]).map(|listing| listing.entries)
        }
    }
    fn stat(&self, path: &str, _ctx: &VirtualContext<'_>) -> Result<Entry, String> {
        #[cfg(target_os = "windows")]
        {
            let _ = path;
            recycle::list()?
                .into_iter()
                .find(|entry| entry.path == path)
                .ok_or_else(|| crate::i18n::t("回收站项目无效").into())
        }
        #[cfg(not(target_os = "windows"))]
        {
            files::entry(Path::new(path))
        }
    }
    fn open(&self, _path: &str) -> Result<(), String> {
        Err(crate::i18n::t("回收站项目请使用“放回原处”或“彻底删除”").into())
    }
    fn remove(&self, paths: &[String]) -> Result<usize, String> {
        trash::delete_permanently(paths, &std::sync::atomic::AtomicBool::new(false))
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
}

static RECENT: RecentResolver = RecentResolver;
static APPLICATIONS: ApplicationResolver = ApplicationResolver;
static TRASH: TrashResolver = TrashResolver;

pub fn resolver_for(path: &str) -> Option<&'static dyn VirtualResolver> {
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
    resolver_for(path).and_then(|resolver| resolver.icon(path))
}

/// One menu contract for Core-owned and plugin-owned VFS adapters.
pub fn menu_json(resolver: &dyn VirtualResolver, paths: &[String]) -> serde_json::Value {
    let items = if paths.is_empty() {
        vec![]
    } else {
        resolver
            .menu(&paths[0])
            .into_iter()
            .filter(|item| {
                paths
                    .iter()
                    .all(|path| resolver.menu(path).iter().any(|other| other.id == item.id))
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
    let provider = resolver_for(path).ok_or("unknown builtin VFS")?;
    if path == RECENT_URI || path == APPLICATIONS_URI || path == recycle::URI {
        return Ok(Entry {
            path: path.into(),
            name: provider.id().into(),
            is_dir: true,
            readonly: false,
            allowed_ops: provider
                .operations(path)
                .into_iter()
                .map(str::to_owned)
                .collect(),
            ..Default::default()
        });
    }
    provider.stat(path, &VirtualContext { recent: &[] })
}
