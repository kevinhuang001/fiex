//! The three Core-owned virtual folders (Recent, Applications, Trash) share
//! one provider surface. They never cross plugin IPC: Core lists, opens,
//! decorates and builds their context menus locally.

use super::{applications, recycle, trash};
#[cfg(not(target_os = "windows"))]
use crate::core::files;
use crate::core::files::Entry;
use serde_json::json;
#[cfg(not(target_os = "windows"))]
use std::path::Path;
#[allow(unused_imports)]
use std::path::PathBuf;

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
    fn list(&self, ctx: &VirtualContext<'_>) -> Result<Vec<Entry>, String>;
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
    fn list(&self, ctx: &VirtualContext<'_>) -> Result<Vec<Entry>, String> {
        Ok(ctx.recent.to_vec())
    }
    fn stat(&self, path: &str, ctx: &VirtualContext<'_>) -> Result<Entry, String> {
        ctx.recent
            .iter()
            .find(|entry| entry.path == path)
            .cloned()
            .ok_or_else(|| crate::i18n::t("最近使用中没有该项目").into())
    }
    fn open(&self, path: &str) -> Result<(), String> {
        crate::core::terminals::open_path(path)
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
            paths: paths.to_vec(),
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
    fn list(&self, _ctx: &VirtualContext<'_>) -> Result<Vec<Entry>, String> {
        applications::list()
    }
    fn stat(&self, path: &str, _ctx: &VirtualContext<'_>) -> Result<Entry, String> {
        applications::list()?
            .into_iter()
            .find(|entry| entry.path == path)
            .ok_or_else(|| crate::i18n::t("应用不存在").into())
    }
    fn open(&self, path: &str) -> Result<(), String> {
        let encoded = path
            .strip_prefix(applications::URI)
            .ok_or_else(|| crate::i18n::t("无法启动应用").to_string())?;
        applications::open(encoded)
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
    fn list(&self, _ctx: &VirtualContext<'_>) -> Result<Vec<Entry>, String> {
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
    if path == RECENT_URI {
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

/// Encode the provider and selection into an opaque menu token so executing a
/// virtual menu item never reinterprets host paths.
pub fn menu_token(resolver: &dyn VirtualResolver, paths: &[String]) -> String {
    use base64::Engine;
    let payload = serde_json::to_vec(&json!({
        "provider": resolver.id(),
        "paths": paths,
    }))
    .unwrap_or_default();
    format!(
        "virtual:{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload)
    )
}

pub fn parse_menu_token(
    token: &str,
) -> Result<(&'static dyn VirtualResolver, Vec<String>), String> {
    use base64::Engine;
    let encoded = token
        .strip_prefix("virtual:")
        .ok_or("unknown menu session")?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|error| error.to_string())?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    let provider = match value["provider"].as_str() {
        Some("core:recent") => &RECENT as &'static dyn VirtualResolver,
        Some("core:applications") => &APPLICATIONS,
        Some("core:trash") => &TRASH,
        _ => return Err("unknown virtual provider".into()),
    };
    let paths =
        serde_json::from_value(value["paths"].clone()).map_err(|error| error.to_string())?;
    Ok((provider, paths))
}

pub fn menu_json(resolver: &dyn VirtualResolver, paths: &[String]) -> serde_json::Value {
    let items = resolver
        .menu(paths.first().map(String::as_str).unwrap_or(""))
        .into_iter()
        .map(|item| {
            json!({
                "id": item.id,
                "kind": "action",
                "title": item.title,
                "icon": item.icon,
                "danger": item.danger,
                "placement": if item.tail { Some("tail") } else { None },
            })
        })
        .collect::<Vec<_>>();
    json!({
        "kind": "builtin",
        "providerId": format!("builtin:{}", resolver.id()),
        "menuSessionId": menu_token(resolver, paths),
        "items": items,
    })
}

pub fn resolve_menu(directory: &str, paths: &[String]) -> Option<serde_json::Value> {
    resolver_for(directory).map(|provider| menu_json(provider, paths))
}

pub fn execute_menu(token: &str, item: &str) -> Result<serde_json::Value, String> {
    let (provider, paths) = parse_menu_token(token)?;
    let action = provider.execute(item, &paths)?;
    Ok(json!({
        "kind": "coreAction",
        "command": action.command,
        "paths": action.paths,
    }))
}

pub fn directory_for(path: &str) -> Option<PathBuf> {
    let _ = path;
    None
}
