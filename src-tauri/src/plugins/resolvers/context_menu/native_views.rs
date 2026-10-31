//! Core-owned view menu contributions. These participate in exactly the same
//! SDK ProviderMatchRule as third-party menus, with trusted view identities.
//! OS-specific operations remain in the owning platform service, not in the
//! generic EventRouter. Plugins never gain Core action ownership.
use crate::core::files::Entry;
use crate::core::volumes::{applications, recycle, trash_vfs, virtual_resolvers};
use crate::plugins::events::Resolver;
use fiex_provider_contracts::{LocationMatch, ProviderDescriptor, ProviderKind, ProviderMatchRule};
use serde_json::{json, Value};
use std::sync::OnceLock;

/// Core-native menus have the same validated descriptor and matcher used for
/// IPC providers. Only the implementation behind the descriptor is native.
struct NativeMenu {
    descriptor: ProviderDescriptor,
    id: &'static str,
    owner: &'static str,
}

fn descriptors() -> &'static [NativeMenu; 3] {
    static REGISTERED: OnceLock<[NativeMenu; 3]> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        fn native(id: &'static str, owner: &'static str) -> NativeMenu {
            let descriptor = ProviderDescriptor {
                id: id.into(),
                kind: ProviderKind::ContextMenu,
                priority: 100,
                r#match: ProviderMatchRule {
                    view: Some(LocationMatch {
                        provider_ids: Some(vec![owner.into()]),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            };
            descriptor
                .validate()
                .expect("invalid built-in menu descriptor");
            NativeMenu {
                descriptor,
                id,
                owner,
            }
        }
        [
            native("core.recent.menu", "core:recent"),
            native("core.applications.menu", "core:applications"),
            native("core.trash.menu", "core:trash"),
        ]
    })
}

fn eligible(provider: &NativeMenu, directory: &str, paths: &[String]) -> bool {
    if paths.is_empty() {
        return false;
    }
    let rule = &provider.descriptor.r#match;
    // A selected item never changes the owner of its containing view.
    // Validate the directory independently, then every selected item.
    let (directory_view, _) = crate::plugins::match_rules::locations_for(directory);
    if !rule.matches_targets(&[json!({"view": directory_view})], 1) {
        return false;
    }
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

fn provider(id: &str, directory: &str, paths: &[String]) -> Result<&'static NativeMenu, String> {
    descriptors()
        .iter()
        .find(|candidate| candidate.descriptor.id == id && eligible(candidate, directory, paths))
        .ok_or_else(|| "native menu provider does not match selection".into())
}

pub(super) fn providers(directory: &str, paths: &[String]) -> Vec<Resolver> {
    descriptors()
        .iter()
        .filter(|candidate| eligible(candidate, directory, paths))
        .map(|candidate| Resolver::builtin(candidate.id, candidate.descriptor.priority))
        .collect()
}

/// Native ownership suppresses only conflicting Core actions. Independent
/// generic Info, Preview, Copy Path and valid clipboard contributions remain
/// eligible; this is not an EventRouter early return for builtin views.
pub(super) fn claimed_actions(directory: &str, paths: &[String]) -> &'static [&'static str] {
    let owner = descriptors()
        .iter()
        .find(|candidate| eligible(candidate, directory, paths));
    match owner.map(|candidate| candidate.owner) {
        Some("core:recent") => &[
            "open",
            "reveal",
            "rename",
            "trash",
            "delete",
            "cut",
            "duplicate",
        ],
        Some("core:applications") => &["open", "rename", "trash", "delete"],
        Some("core:trash") => &[
            "open",
            "restore",
            "delete",
            "trash",
            "rename",
            "cut",
            "duplicate",
        ],
        _ => &[],
    }
}

pub(super) fn owns(id: &str) -> bool {
    descriptors()
        .iter()
        .any(|provider| provider.descriptor.id == id)
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
        _ => Vec::new(),
    }
}

pub(super) fn resolve(id: &str, directory: &str, paths: &[String]) -> Result<Value, String> {
    let owner = provider(id, directory, paths)?.owner;
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

pub(super) fn execute(
    id: &str,
    item_id: &str,
    directory: &str,
    paths: &[String],
) -> Result<NativeAction, String> {
    let owner = provider(id, directory, paths)?.owner;
    match (owner, item_id) {
        ("core:recent", "open") => Ok(NativeAction {
            command: "open",
            paths: paths
                .iter()
                .map(|path| crate::core::recent::source_for(path))
                .collect::<Result<Vec<_>, _>>()?,
        }),
        ("core:recent", "reveal") => {
            for path in paths {
                virtual_resolvers::recent_reveal_source(path)
                    .ok_or_else(|| crate::i18n::t("无法在文件管理器中显示该位置").to_string())?;
            }
            Ok(NativeAction {
                command: "reveal",
                paths: paths.to_vec(),
            })
        }
        ("core:applications", "open") => Ok(NativeAction {
            command: "open",
            paths: paths.to_vec(),
        }),
        ("core:applications", "rename" | "trash") => {
            if paths.iter().any(|path| !applications::is_user(path)) {
                return Err("only stored application entries can be modified".into());
            }
            Ok(NativeAction {
                command: if item_id == "rename" {
                    "rename"
                } else {
                    "trashApplications"
                },
                paths: paths.to_vec(),
            })
        }
        ("core:applications", "delete") => Err(crate::i18n::t("系统应用不能在 Fiex 中删除").into()),
        ("core:trash", "restore") => Ok(NativeAction {
            command: "restore",
            paths: paths.to_vec(),
        }),
        ("core:trash", "delete") => Ok(NativeAction {
            command: "delete",
            paths: paths.to_vec(),
        }),
        _ => Err("unknown native menu action".into()),
    }
}

#[cfg(test)]
mod tests;
