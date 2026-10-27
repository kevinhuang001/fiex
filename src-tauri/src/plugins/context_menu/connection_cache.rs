//! Connection-cache menu group. Remote selections only; the action set is
//! owned by the connection adapter and unchanged by capability gating.

use super::file_ops::{item, BuiltinResolver, Selection};
use crate::plugin_file_capabilities::FileCapabilities;
use serde_json::{json, Value};

pub(super) struct ConnectionCache;

impl BuiltinResolver for ConnectionCache {
    fn id(&self) -> &'static str {
        "builtin:connection-cache"
    }
    fn priority(&self) -> i64 {
        98
    }
    fn available(&self, selection: &Selection<'_>, _: &FileCapabilities) -> bool {
        !selection.paths.is_empty()
            && selection
                .paths
                .iter()
                .all(|path| crate::remote::is_remote(path))
    }
    fn items(&self, selection: &Selection<'_>, _: &FileCapabilities) -> Vec<Value> {
        let entries = selection.entries;
        let pinned = entries.iter().all(|entry| entry.pinned);
        let cached = entries.iter().all(|entry| entry.has_cache);
        let conflict = entries
            .iter()
            .any(|entry| entry.sync.as_deref() == Some("conflict"));
        let mut pin = if pinned {
            item("remote.unpin", "取消保留在本地", "pin-off", "")
        } else {
            item("remote.pin", "始终保留在本地", "cloud-download", "")
        };
        let label = crate::remote::connection_for_path(&selection.paths[0])
            .map(|(connection, _)| crate::i18n::t("{0} 触发同步").replace("{0}", &connection.name))
            .unwrap_or_else(|| crate::i18n::t("触发同步").into());
        if !pinned {
            activity(&mut pin, &label, "sync", true);
        }
        let mut items = vec![pin];
        if !cached {
            let mut cache = item("remote.cache", "缓存到本地", "cloud-download", "");
            activity(&mut cache, crate::i18n::t("正在缓存…"), "transfer", false);
            items.push(cache);
        } else if !entries.iter().any(|entry| {
            entry.pinned || matches!(entry.sync.as_deref(), Some("conflict" | "pending"))
        }) {
            items.push(item("remote.uncache", "取消缓存", "cloud-off", ""));
        }
        if conflict {
            items.push(item(
                "remote.keepLocal",
                "用本地版本覆盖云端",
                "cloud-check",
                "",
            ));
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

fn activity(item: &mut Value, label: &str, kind: &str, quiet: bool) {
    item["activity"] = json!({"label":label,"kind":kind,"quiet":quiet});
}
