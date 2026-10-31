//! Native view-owned ContextMenu behavior. Each view implements the same
//! ContextMenu contract and no generic menu dispatcher knows its URI scheme.
use super::{NativeAction, NativeViewMenuProvider};
use crate::core::files::Entry;
use crate::core::volumes::{applications, recycle, trash_vfs, virtual_resolvers};
use serde_json::{json, Value};

fn item(id: &str, title: String, icon: &str, danger: bool, tail: bool) -> Value {
    json!({
        "id": id, "kind": "action", "title": title, "icon": icon,
        "danger": danger, "placement": if tail { Some("tail") } else { None }
    })
}

pub(super) struct RecentMenu;
impl NativeViewMenuProvider for RecentMenu {
    fn id(&self) -> &'static str {
        "core.recent.menu"
    }

    fn owner(&self) -> &'static str {
        "core:recent"
    }

    fn claimed_actions(&self) -> &'static [&'static str] {
        &["open", "reveal", "rename", "trash", "delete", "cut", "duplicate"]
    }

    fn items(&self, path: &str) -> Vec<Value> {
        let mut items = vec![item(
            "open",
            crate::i18n::t("打开").into(),
            "open",
            false,
            false,
        )];
        if virtual_resolvers::recent_reveal_source(path).is_some() {
            items.push(item(
                "reveal",
                crate::i18n::t("打开文件所在位置").into(),
                "reveal",
                false,
                false,
            ));
        }
        items
    }

    fn execute(&self, id: &str, paths: &[String]) -> Result<NativeAction, String> {
        match id {
            "open" => Ok(NativeAction {
                command: "open",
                paths: paths
                    .iter()
                    .map(|path| crate::core::recent::source_for(path))
                    .collect::<Result<Vec<_>, _>>()?,
            }),
            "reveal" => {
                for path in paths {
                    virtual_resolvers::recent_reveal_source(path).ok_or_else(|| {
                        crate::i18n::t("无法在文件管理器中显示该位置").to_string()
                    })?;
                }
                Ok(NativeAction {
                    command: "reveal",
                    paths: paths.to_vec(),
                })
            }
            _ => Err("unknown Recent menu action".into()),
        }
    }
}

pub(super) struct ApplicationsMenu;
impl NativeViewMenuProvider for ApplicationsMenu {
    fn id(&self) -> &'static str {
        "core.applications.menu"
    }

    fn owner(&self) -> &'static str {
        "core:applications"
    }

    fn claimed_actions(&self) -> &'static [&'static str] {
        &["open", "rename", "trash", "delete"]
    }

    fn items(&self, path: &str) -> Vec<Value> {
        let user = applications::is_user(path);
        let launch = path != applications::URI && (!user || user_application(path));
        let mut items = vec![item(
            "open",
            crate::i18n::t(if launch { "启动应用" } else { "打开" }).into(),
            "open",
            false,
            false,
        )];
        if user {
            items.push(item(
                "rename",
                crate::i18n::t("重命名").into(),
                "pencil",
                false,
                false,
            ));
            items.push(item(
                "trash",
                crate::i18n::t(trash_title()).into(),
                "trash",
                true,
                true,
            ));
        }
        items
    }

    fn execute(&self, id: &str, paths: &[String]) -> Result<NativeAction, String> {
        match id {
            "open" => Ok(NativeAction {
                command: "open",
                paths: paths.to_vec(),
            }),
            "rename" | "trash" => {
                if paths.iter().any(|path| !applications::is_user(path)) {
                    return Err("only stored application entries can be modified".into());
                }
                Ok(NativeAction {
                    command: if id == "rename" {
                        "rename"
                    } else {
                        "trashApplications"
                    },
                    paths: paths.to_vec(),
                })
            }
            "delete" => Err(crate::i18n::t("系统应用不能在 Fiex 中删除").into()),
            _ => Err("unknown Applications menu action".into()),
        }
    }
}

fn user_application(path: &str) -> bool {
    applications::stored_entry(path).is_ok_and(|entry| user_application_entry(&entry))
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

pub(super) struct TrashMenu;
impl NativeViewMenuProvider for TrashMenu {
    fn id(&self) -> &'static str {
        "core.trash.menu"
    }

    fn owner(&self) -> &'static str {
        "core:trash"
    }

    fn claimed_actions(&self) -> &'static [&'static str] {
        &[
            "open", "restore", "delete", "trash", "rename", "cut", "duplicate",
        ]
    }

    fn items(&self, path: &str) -> Vec<Value> {
        if path == trash_vfs::URI || path == recycle::URI {
            return Vec::new();
        }
        vec![
            item(
                "restore",
                crate::i18n::t("放回原处").into(),
                "restore",
                false,
                false,
            ),
            item(
                "delete",
                crate::i18n::t("彻底删除").into(),
                "trash",
                true,
                true,
            ),
        ]
    }

    fn execute(&self, id: &str, paths: &[String]) -> Result<NativeAction, String> {
        match id {
            "restore" => Ok(NativeAction {
                command: "restore",
                paths: paths.to_vec(),
            }),
            "delete" => Ok(NativeAction {
                command: "delete",
                paths: paths.to_vec(),
            }),
            _ => Err("unknown Trash menu action".into()),
        }
    }
}
