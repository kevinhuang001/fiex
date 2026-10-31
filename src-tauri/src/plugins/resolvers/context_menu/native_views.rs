//! Core-owned view menu contributions. These participate in exactly the same
//! SDK ProviderMatchRule as third-party menus, with trusted view identities.
//! OS-specific operations remain in the owning platform service, not in the
//! generic EventRouter. Plugins never gain Core action ownership.
use crate::core::files::Entry;
use crate::core::volumes::{applications, recycle, trash_vfs, virtual_resolvers};
use crate::plugins::events::Resolver;
use crate::plugin_sdk::provider_matching::{LocationMatch, ProviderMatchRule};
use serde_json::{json, Value};

struct Descriptor {
    id: &'static str,
    owner: &'static str,
    priority: i64,
}

const PROVIDERS: [Descriptor; 3] = [
    Descriptor {
        id: "core.recent.menu",
        owner: "core:recent",
        priority: 100,
    },
    Descriptor {
        id: "core.applications.menu",
        owner: "core:applications",
        priority: 100,
    },
    Descriptor {
        id: "core.trash.menu",
        owner: "core:trash",
        priority: 100,
    },
];

fn eligible(provider: &Descriptor, paths: &[String]) -> bool {
    if paths.is_empty() {
        return false;
    }
    let rule = ProviderMatchRule {
        view: Some(LocationMatch {
            provider_ids: Some(vec![provider.owner.into()]),
            ..Default::default()
        }),
        ..Default::default()
    };
    // A mixed-source selection cannot inherit the menu rights of its first
    // member. All selected view identities must be derived by Core.
    let targets = paths
        .iter()
        .map(|path| {
            let (view, _) = crate::plugins::match_rules::locations_for(path);
            json!({"view": view})
        })
        .collect::<Vec<_>>();
    rule.matches_targets(&targets, paths.len())
}

fn provider(id: &str, paths: &[String]) -> Result<&'static Descriptor, String> {
    PROVIDERS
        .iter()
        .find(|candidate| candidate.id == id && eligible(candidate, paths))
        .ok_or_else(|| "native menu provider does not match selection".into())
}

pub(super) fn providers(paths: &[String]) -> Vec<Resolver> {
    PROVIDERS
        .iter()
        .filter(|candidate| eligible(candidate, paths))
        .map(|candidate| Resolver::builtin(candidate.id, candidate.priority))
        .collect()
}

pub(super) fn owns(id: &str) -> bool {
    PROVIDERS.iter().any(|provider| provider.id == id)
}

fn item(id: &str, title: String, icon: &str, danger: bool, tail: bool) -> Value {
    json!({
        "id": id, "kind": "action", "title": title, "icon": icon,
        "danger": danger, "placement": if tail { Some("tail") } else { None }
    })
}

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

fn menu_for(owner: &str, path: &str) -> Vec<Value> {
    match owner {
        "core:recent" => {
            let mut items = vec![item("open", crate::i18n::t("打开").into(), "open", false, false)];
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
        "core:applications" => {
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
        "core:trash" => {
            if path == trash_vfs::URI || path == recycle::URI {
                return Vec::new();
            }
            vec![
                item("restore", crate::i18n::t("放回原处").into(), "restore", false, false),
                item("delete", crate::i18n::t("彻底删除").into(), "trash", true, true),
            ]
        }
        _ => Vec::new(),
    }
}

pub(super) fn resolve(id: &str, paths: &[String]) -> Result<Value, String> {
    let owner = provider(id, paths)?.owner;
    let offered = menu_for(owner, &paths[0]);
    let intersection = offered
        .into_iter()
        .filter(|item| {
            paths.iter().skip(1).all(|path| {
                menu_for(owner, path)
                    .iter()
                    .any(|other| other["id"] == item["id"])
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({"items": intersection}))
}

pub(super) struct NativeAction {
    pub command: &'static str,
    pub paths: Vec<String>,
}

pub(super) fn execute(id: &str, item_id: &str, paths: &[String]) -> Result<NativeAction, String> {
    let owner = provider(id, paths)?.owner;
    match (owner, item_id) {
        ("core:recent", "open") => Ok(NativeAction {
            command: "open",
            paths: paths.iter().map(|path| crate::core::recent::source_for(path))
                .collect::<Result<Vec<_>, _>>()?,
        }),
        ("core:recent", "reveal") => {
            for path in paths {
                virtual_resolvers::recent_reveal_source(path).ok_or_else(|| {
                    crate::i18n::t("无法在文件管理器中显示该位置").to_string()
                })?;
            }
            Ok(NativeAction { command: "reveal", paths: paths.to_vec() })
        }
        ("core:applications", "open") => Ok(NativeAction {
            command: "open", paths: paths.to_vec(),
        }),
        ("core:applications", "rename" | "trash") => {
            if paths.iter().any(|path| !applications::is_user(path)) {
                return Err("only stored application entries can be modified".into());
            }
            Ok(NativeAction {
                command: if item_id == "rename" { "rename" } else { "trashApplications" },
                paths: paths.to_vec(),
            })
        }
        ("core:applications", "delete") => {
            Err(crate::i18n::t("系统应用不能在 Fiex 中删除").into())
        }
        ("core:trash", "restore") => Ok(NativeAction {
            command: "restore", paths: paths.to_vec(),
        }),
        ("core:trash", "delete") => Ok(NativeAction {
            command: "delete", paths: paths.to_vec(),
        }),
        _ => Err("unknown native menu action".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_descriptors_are_disjoint_and_not_plugin_privileged() {
        let applications = vec!["applications://c3lzdGVtLWFwcA".into()];
        let recent = vec!["recent://not-saved".into()];
        let root = vec!["/tmp/example".into()];
        assert_eq!(providers(&applications)[0].id, "core.applications.menu");
        assert_eq!(providers(&recent)[0].id, "core.recent.menu");
        assert!(providers(&root).is_empty());
        assert!(providers(&[applications[0].clone(), recent[0].clone()]).is_empty());
    }

    #[test]
    fn system_app_is_launch_only_and_user_entries_are_protected() {
        let system = vec!["applications://c3lzdGVtLWFwcA".into()];
        let value = resolve("core.applications.menu", &system).unwrap();
        assert_eq!(value["items"].as_array().unwrap().len(), 1);
        assert_eq!(value["items"][0]["id"], "open");
        assert!(execute("core.applications.menu", "rename", &system).is_err());
        let user = vec![format!("{}user/dXNlci1hcHA", applications::URI)];
        let value = resolve("core.applications.menu", &user).unwrap();
        assert_eq!(value["items"].as_array().unwrap().len(), 3);
        assert_eq!(execute("core.applications.menu", "trash", &user).unwrap().command, "trashApplications");
    }

    #[test]
    fn mixed_app_selections_have_only_shared_actions() {
        let selections = vec![
            "applications://c3lzdGVtLWFwcA".into(),
            format!("{}user/dXNlci1hcHA", applications::URI),
        ];
        let value = resolve("core.applications.menu", &selections).unwrap();
        assert_eq!(value["items"].as_array().unwrap().len(), 1);
        assert_eq!(value["items"][0]["id"], "open");
    }
}
