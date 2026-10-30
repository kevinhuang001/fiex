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
    /// Reveal an entry in the platform file manager. Only views that can map a
    /// virtual identifier onto a real host location implement this.
    fn reveal(&self, _path: &str) -> Result<(), String> {
        Err("Unsupported: builtin view has no reveal action".into())
    }
}

/// A Recent alias can be revealed only when its original resource still exists
/// on a local filesystem. Remote, USBFS and plugin-mount sources are excluded:
/// their display paths are not locations a host file manager can select.
fn recent_reveal_source(alias: &str) -> Option<String> {
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

fn recent_menu_items(reveal: bool) -> Vec<VirtualMenuItem> {
    let mut items = vec![VirtualMenuItem {
        id: "open",
        title: crate::i18n::t("打开").into(),
        icon: "open",
        danger: false,
        tail: false,
    }];
    // The reveal row is part of the first Core snapshot: whether a Recent
    // source is a real local file is decided here, not by plugin IPC.
    if reveal {
        items.push(VirtualMenuItem {
            id: "reveal",
            title: crate::i18n::t("打开文件所在位置").into(),
            icon: "reveal",
            danger: false,
            tail: false,
        });
    }
    items
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
    fn menu(&self, path: &str) -> Vec<VirtualMenuItem> {
        recent_menu_items(recent_reveal_source(path).is_some())
    }
    fn execute(&self, action: &str, paths: &[String]) -> Result<VirtualAction, String> {
        match action {
            "open" => Ok(VirtualAction {
                command: "open",
                paths: paths
                    .iter()
                    .map(|path| crate::core::recent::source_for(path))
                    .collect::<Result<Vec<_>, _>>()?,
            }),
            "reveal" => {
                for path in paths {
                    recent_reveal_source(path).ok_or_else(|| {
                        crate::i18n::t("无法在文件管理器中显示该位置").to_string()
                    })?;
                }
                Ok(VirtualAction {
                    command: "reveal",
                    // Keep aliases across IPC; `reveal_path` resolves them in
                    // Core and never hands the source path to the webview.
                    paths: paths.to_vec(),
                })
            }
            _ => Err("unknown recent action".into()),
        }
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

/// A user-store entry is an application when it is a file
/// (shortcut/executable) or an app-bundle directory. Plain folders stay
/// navigable and keep the "打开" verb.
fn user_application(path: &str) -> bool {
    applications::is_user(path)
        && applications::stored_entry(path).is_ok_and(|entry| user_application_entry(&entry))
}

fn user_application_entry(entry: &Entry) -> bool {
    !entry.is_dir || entry.extension == "app"
}

fn trash_title() -> &'static str {
    if cfg!(windows) {
        "移到回收站"
    } else {
        "移到废纸篓"
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
        // The user store is an application library: its files launch, rename or
        // move to the platform Trash; its folders open, rename or move to the
        // Trash. A system entry is launch-only until it is mapped into a folder,
        // where delete removes the mapping and returns it to the root.
        let user = applications::is_user(path);
        let launch = path != APPLICATIONS_URI && (!user || user_application(path));
        let mut items = vec![VirtualMenuItem {
            id: "open",
            title: if launch {
                crate::i18n::t("启动应用").into()
            } else {
                crate::i18n::t("打开").into()
            },
            icon: "open",
            danger: false,
            tail: false,
        }];
        if user {
            items.push(VirtualMenuItem {
                id: "rename",
                title: crate::i18n::t("重命名").into(),
                icon: "pencil",
                danger: false,
                tail: false,
            });
            items.push(VirtualMenuItem {
                id: "trash",
                title: crate::i18n::t(trash_title()).into(),
                icon: "trash",
                danger: true,
                tail: true,
            });
        } else if applications::is_mapped(path) {
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
            // `open_path` forwards an Applications URI to
            // `applications::open()`, which launches system entries through
            // `platform_open()` and opens user shortcuts from the private store.
            "open" => Ok(VirtualAction {
                command: "open",
                paths: paths.to_vec(),
            }),
            "rename" => {
                if paths.iter().any(|path| !applications::is_user(path)) {
                    return Err("only stored application entries can be renamed".into());
                }
                Ok(VirtualAction {
                    command: "rename",
                    paths: paths.to_vec(),
                })
            }
            "trash" => {
                if paths.iter().any(|path| !applications::is_user(path)) {
                    return Err("only stored application entries can move to the Trash".into());
                }
                Ok(VirtualAction {
                    command: "trashApplications",
                    paths: paths.to_vec(),
                })
            }
            "delete" => {
                if paths
                    .iter()
                    .all(|path| applications::is_system_app(path) && applications::is_mapped(path))
                {
                    return Ok(VirtualAction {
                        command: "removeApplications",
                        paths: paths.to_vec(),
                    });
                }
                Ok(VirtualAction {
                    command: "delete",
                    paths: paths.to_vec(),
                })
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn system_path() -> String {
        format!("{}c3lzdGVtLWFwcA", applications::URI)
    }

    fn user_path() -> String {
        format!("{}user/dXNlci1hcHA", applications::URI)
    }

    #[test]
    fn system_application_entry_offers_an_explicit_launch_verb() {
        let path = system_path();
        let items = ApplicationView.menu(&path);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "open");
        assert_eq!(items[0].title, crate::i18n::t("启动应用"));
        assert_eq!(items[0].icon, "open");
        assert!(!items[0].danger);
        assert!(!items[0].tail);
        // The Core action keeps the URI: `open_path` forwards it to
        // `applications::open()`/`platform_open()`.
        let action = ApplicationView
            .execute("open", std::slice::from_ref(&path))
            .unwrap();
        assert_eq!(action.command, "open");
        assert_eq!(action.paths, vec![path]);
    }

    #[test]
    fn user_application_entry_offers_launch_rename_and_trash() {
        let path = user_path();
        let items = ApplicationView.menu(&path);
        assert_eq!(
            items.iter().map(|item| item.id).collect::<Vec<_>>(),
            ["open", "rename", "trash"]
        );
        assert_eq!(items[0].title, crate::i18n::t("打开"));
        assert_eq!(items[1].title, crate::i18n::t("重命名"));
        assert_eq!(items[2].title, crate::i18n::t(trash_title()));
        assert!(items[2].danger);
        assert!(items[2].tail);
        let action = ApplicationView
            .execute("open", std::slice::from_ref(&path))
            .unwrap();
        assert_eq!(action.command, "open");
        assert_eq!(action.paths, vec![path.clone()]);
        assert_eq!(
            ApplicationView
                .execute("rename", std::slice::from_ref(&path))
                .unwrap()
                .command,
            "rename"
        );
        assert_eq!(
            ApplicationView
                .execute("trash", std::slice::from_ref(&path))
                .unwrap()
                .command,
            "trashApplications"
        );
    }

    #[test]
    fn applications_root_is_a_folder_not_an_installed_application() {
        let items = ApplicationView.menu(APPLICATIONS_URI);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, crate::i18n::t("打开"));
    }

    #[test]
    fn stored_user_entries_launch_only_files_or_app_bundles() {
        let shortcut = Entry::default();
        assert!(user_application_entry(&shortcut));

        let folder = Entry {
            is_dir: true,
            ..Default::default()
        };
        assert!(!user_application_entry(&folder));

        let bundle = Entry {
            is_dir: true,
            extension: "app".into(),
            ..Default::default()
        };
        assert!(user_application_entry(&bundle));

        assert!(!user_application(&format!(
            "{}loose",
            applications::USER_PREFIX
        )));
        assert!(!user_application("applications://c3lzdGVt"));
    }

    #[test]
    fn a_mixed_selection_keeps_the_shared_open_item() {
        let system = system_path();
        let user = user_path();
        let menu = menu_json(&ApplicationView, &[system, user]);
        let items = menu["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], "open");
    }

    #[test]
    fn recent_reveal_only_accepts_existing_local_files() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("recent.txt");
        std::fs::write(&file, b"x").unwrap();
        let file = file.to_string_lossy().into_owned();
        assert!(revealable_source(&file));
        // Directories, missing files, plugin mounts and opaque Recent aliases
        // cannot be selected by a host file manager.
        assert!(!revealable_source(&directory.path().to_string_lossy()));
        assert!(!revealable_source("/fiex/definitely/missing/recent.txt"));
        assert!(!revealable_source("fiex://mount/example/recent.txt"));
        assert!(!revealable_source(&format!("{}missing", RECENT_URI)));
    }

    #[test]
    fn recent_reveal_row_has_the_expected_contract() {
        let items = recent_menu_items(true);
        assert_eq!(
            items.iter().map(|item| item.id).collect::<Vec<_>>(),
            ["open", "reveal"]
        );
        assert_eq!(items[1].title, crate::i18n::t("打开文件所在位置"));
        assert_eq!(items[1].icon, "reveal");
        assert!(!items[1].danger);
        assert!(!items[1].tail);
        let plain = recent_menu_items(false);
        assert_eq!(
            plain.iter().map(|item| item.id).collect::<Vec<_>>(),
            ["open"]
        );
    }

    #[test]
    fn an_unregistered_recent_alias_has_no_reveal_action() {
        let alias = format!("{}missing-{}", RECENT_URI, std::process::id());
        let items = RecentView.menu(&alias);
        assert_eq!(
            items.iter().map(|item| item.id).collect::<Vec<_>>(),
            ["open"]
        );
        assert!(recent_reveal_source(&alias).is_none());
        assert!(RecentView
            .execute("reveal", std::slice::from_ref(&alias))
            .is_err());
        assert!(RecentView.reveal(&alias).is_err());
    }
}
