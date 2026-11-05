//! Connection-cache menu group. Remote selections only; the action set is
//! owned by the connection adapter and unchanged by capability gating.

use super::file_ops::{item, Selection};
use fiex_provider_contracts::{MenuActivity, MenuActivityKind, MenuItem};
use fiex_resource::FileCapabilities;

pub(super) struct ConnectionCache;

impl ConnectionCache {
    pub(super) fn items(&self, selection: &Selection<'_>, _: &FileCapabilities) -> Vec<MenuItem> {
        let entries = selection.entries;
        let can_cache = entries.len() == selection.paths.len()
            && entries.iter().all(|entry| {
                let required = if entry.is_dir { "list" } else { "read" };
                entry
                    .allowed_ops
                    .iter()
                    .any(|operation| operation == required)
            });
        let pinned = entries.iter().all(|entry| entry.pinned);
        let has_cache =
            entries.len() == selection.paths.len() && entries.iter().all(|entry| entry.has_cache);
        let complete = entries.len() == selection.paths.len()
            && entries
                .iter()
                .all(|entry| matches!(entry.sync.as_deref(), Some("synced" | "pinned")));
        let has_pending_or_conflict = entries
            .iter()
            .any(|entry| matches!(entry.sync.as_deref(), Some("pending" | "conflict")));
        let conflicts = entries
            .iter()
            .filter(|entry| entry.sync.as_deref() == Some("conflict"))
            .collect::<Vec<_>>();
        let mut items = Vec::new();
        if pinned {
            items.push(item("remote.unpin", "取消保留在本地", "pin-off", ""));
        } else if can_cache {
            let mut pin = item("remote.pin", "始终保留在本地", "cloud-download", "");
            let label = crate::remote::connection_for_path(&selection.paths[0])
                .map(|(connection, _)| {
                    crate::i18n::t("{0} 触发同步").replace("{0}", &connection.name)
                })
                .unwrap_or_else(|| crate::i18n::t("触发同步").into());
            activity(&mut pin, &label, MenuActivityKind::Sync, true);
            items.push(pin);
        }
        if !complete && !has_pending_or_conflict && can_cache {
            let mut cache = item("remote.cache", "缓存到本地", "cloud-download", "");
            activity(
                &mut cache,
                crate::i18n::t("正在缓存…"),
                MenuActivityKind::Transfer,
                false,
            );
            items.push(cache);
        }
        if has_cache
            && !entries.iter().any(|entry| {
                entry.pinned || matches!(entry.sync.as_deref(), Some("conflict" | "pending"))
            })
        {
            items.push(item("remote.uncache", "取消缓存", "cloud-off", ""));
        }
        if !conflicts.is_empty()
            && conflicts.iter().all(|entry| {
                entry
                    .allowed_ops
                    .iter()
                    .any(|operation| operation == "write")
            })
        {
            items.push(item(
                "remote.keepLocal",
                "用本地版本覆盖云端",
                "cloud-check",
                "",
            ));
        }
        if !conflicts.is_empty()
            && conflicts.iter().all(|entry| {
                entry
                    .allowed_ops
                    .iter()
                    .any(|operation| operation == "read")
            })
        {
            items.push(item(
                "remote.keepRemote",
                "保留本地备份并采用云端版本",
                "cloud-check",
                "",
            ));
        }
        items
    }
}

fn activity(item: &mut MenuItem, label: &str, kind: MenuActivityKind, quiet: bool) {
    item.activity = Some(MenuActivity {
        label: label.into(),
        kind,
        quiet,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(allowed_ops: &[&str]) -> crate::core::files::Entry {
        crate::core::files::Entry {
            path: "fiex://connection-cache-test/file".into(),
            name: "file".into(),
            allowed_ops: allowed_ops
                .iter()
                .map(|operation| operation.to_string())
                .collect(),
            ..Default::default()
        }
    }

    fn item_ids(items: &[MenuItem]) -> Vec<&str> {
        items.iter().map(|item| item.id.as_str()).collect()
    }

    #[test]
    fn cache_actions_require_the_provider_read_capability() {
        let paths = ["fiex://connection-cache-test/file".to_string()];
        let mut file = entry(&["stat", "read"]);
        let selection = Selection {
            paths: &paths,
            entries: std::slice::from_ref(&file),
            all_local: false,
            preflight: super::super::builtin::MenuPreflightAvailability::default(),
        };
        let items = ConnectionCache.items(&selection, &FileCapabilities::default());
        let ids = item_ids(&items);
        assert!(ids.contains(&"remote.cache") && ids.contains(&"remote.pin"));
        assert!(!ids.contains(&"remote.uncache") && !ids.contains(&"remote.unpin"));

        file.allowed_ops = vec!["stat".into()];
        let selection = Selection {
            paths: &paths,
            entries: std::slice::from_ref(&file),
            all_local: false,
            preflight: super::super::builtin::MenuPreflightAvailability::default(),
        };
        let items = ConnectionCache.items(&selection, &FileCapabilities::default());
        let ids = item_ids(&items);
        assert!(ids.is_empty());
    }

    #[test]
    fn partial_directory_can_finish_or_release_its_cache() {
        let paths = ["fiex://connection-cache-test/folder".to_string()];
        let mut folder = entry(&["list"]);
        folder.is_dir = true;
        folder.has_cache = true;
        folder.sync = Some("partial".into());
        let selection = Selection {
            paths: &paths,
            entries: std::slice::from_ref(&folder),
            all_local: false,
            preflight: super::super::builtin::MenuPreflightAvailability::default(),
        };

        let items = ConnectionCache.items(&selection, &FileCapabilities::default());
        let ids = item_ids(&items);
        assert!(ids.contains(&"remote.cache"));
        assert!(ids.contains(&"remote.uncache"));
    }

    #[test]
    fn dirty_or_conflicting_nodes_never_offer_a_second_download() {
        let paths = ["fiex://connection-cache-test/file".to_string()];
        for state in ["pending", "conflict"] {
            let mut file = entry(&["read", "write"]);
            file.sync = Some(state.into());
            let selection = Selection {
                paths: &paths,
                entries: std::slice::from_ref(&file),
                all_local: false,
                preflight: super::super::builtin::MenuPreflightAvailability::default(),
            };

            let items = ConnectionCache.items(&selection, &FileCapabilities::default());
            let ids = item_ids(&items);
            assert!(!ids.contains(&"remote.cache"));
        }
    }

    #[test]
    fn conflict_actions_follow_the_entry_capabilities() {
        let paths = ["fiex://connection-cache-test/file".to_string()];
        let mut file = entry(&["stat", "read"]);
        file.sync = Some("conflict".into());
        file.has_cache = true;
        let selection = Selection {
            paths: &paths,
            entries: std::slice::from_ref(&file),
            all_local: false,
            preflight: super::super::builtin::MenuPreflightAvailability::default(),
        };
        let items = ConnectionCache.items(&selection, &FileCapabilities::default());
        let ids = item_ids(&items);
        assert!(ids.contains(&"remote.keepRemote"));
        assert!(!ids.contains(&"remote.keepLocal") && !ids.contains(&"remote.uncache"));

        file.allowed_ops.push("write".into());
        let selection = Selection {
            paths: &paths,
            entries: std::slice::from_ref(&file),
            all_local: false,
            preflight: super::super::builtin::MenuPreflightAvailability::default(),
        };
        let items = ConnectionCache.items(&selection, &FileCapabilities::default());
        let ids = item_ids(&items);
        assert!(ids.contains(&"remote.keepLocal") && ids.contains(&"remote.keepRemote"));
    }
}
