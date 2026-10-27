//! Builtin menu policy. The frontend receives data, never provider closures.
use crate::plugin_events::Provider;
use serde_json::{json, Value};

pub(super) struct Context {
    pub directory: String,
    pub paths: Vec<String>,
    pub entries: Vec<crate::files::Entry>,
    pub read_only: bool,
    pub in_trash: bool,
}
impl Context {
    pub fn load(directory: &str, paths: &[String]) -> Result<Self, String> {
        fn entry(path: &str) -> Result<crate::files::Entry, String> {
            if crate::remote::is_remote(path) {
                crate::remote::stat(path)
            } else if crate::mounts::handles(path) {
                crate::plugin_vfs::stat(path)
            } else {
                crate::files::entry(std::path::Path::new(path))
            }
        }
        let entries = paths
            .iter()
            .map(|path| entry(path))
            .collect::<Result<Vec<_>, _>>()?;
        let read_only = if directory == "recent://" {
            true
        } else {
            entry(directory)?.readonly
        };
        let in_trash = !crate::mounts::handles(directory)
            && crate::volumes::trash_path()
                .is_some_and(|trash| std::path::Path::new(directory).starts_with(trash));
        Ok(Self {
            directory: directory.into(),
            paths: paths.to_vec(),
            entries,
            read_only,
            in_trash,
        })
    }
}
const IDS: &[&str] = &[
    "builtin:opening",
    "builtin:tags",
    "builtin:connection-cache",
    "builtin:rename",
    "builtin:batch-rename",
    "builtin:clipboard",
    "builtin:terminal",
    "builtin:copy-path",
    "builtin:trash",
];
pub(super) fn providers() -> Vec<Provider> {
    IDS.iter()
        .enumerate()
        .map(|(index, id)| Provider::builtin(id, 100 - index as i64))
        .collect()
}
fn item(id: &str, title: &'static str, icon: &str, shortcut: &str, disabled: bool) -> Value {
    json!({"id":id,"kind":"action","title":crate::i18n::t(title),"icon":icon,"shortcut":shortcut,"disabled":disabled})
}
pub(super) fn resolve(id: &str, ctx: &Context) -> Result<Value, String> {
    let many = ctx.paths.len() > 1;
    let ro = ctx.read_only;
    let items = match id {
        "builtin:opening" => {
            let mut items = vec![item("open", "打开", "open", "open", false)];
            if !many {
                items.extend([
                    item("preview", "快速查看", "eye", "quickLook", false),
                    item("info", "显示简介", "info", "info", false),
                ]);
            }
            items
        }
        "builtin:tags" if cfg!(target_os = "macos") => vec![item("tags", "标签…", "tags", "", ro)],
        "builtin:tags" => vec![],
        "builtin:connection-cache" => connection_items(ctx),
        "builtin:rename" if !many => vec![item("rename", "重新命名", "pencil", "rename", ro)],
        "builtin:batch-rename" if many => {
            vec![item("batchRename", "批量重命名…", "pencil", "", ro)]
        }
        "builtin:rename" | "builtin:batch-rename" => vec![],
        "builtin:clipboard" => vec![
            item("copy", "复制", "copy", "copy", false),
            item("cut", "剪切", "scissors", "cut", ro),
            item("duplicate", "创建副本", "copy", "duplicate", ro),
        ],
        "builtin:terminal" => vec![item("terminal", "Open in Terminal", "terminal", "", ro)],
        "builtin:copy-path" => vec![item("copyPath", "复制路径", "copy", "", false)],
        "builtin:trash" => {
            let mut items = if ctx.in_trash {
                vec![
                    item("restore", "放回原处", "restore", "", false),
                    item("delete", "彻底删除", "trash", "", ro),
                ]
            } else {
                vec![item("trash", "移到废纸篓", "trash", "trash", ro)]
            };
            for item in &mut items {
                item["placement"] = json!("tail");
                item["danger"] = json!(item["id"] != "restore");
            }
            items
        }
        _ => return Err("unknown builtin menu provider".into()),
    };
    Ok(json!({"items":items}))
}
fn activity(item: &mut Value, label: &str, kind: &str, quiet: bool) {
    item["activity"] = json!({"label":label,"kind":kind,"quiet":quiet});
}
fn connection_items(ctx: &Context) -> Vec<Value> {
    if !ctx.paths.iter().all(|path| crate::remote::is_remote(path)) {
        return vec![];
    }
    let pinned = ctx.entries.iter().all(|entry| entry.pinned);
    let cached = ctx.entries.iter().all(|entry| entry.has_cache);
    let conflict = ctx
        .entries
        .iter()
        .any(|entry| entry.sync.as_deref() == Some("conflict"));
    let mut pin = if pinned {
        item("remote.unpin", "取消保留在本地", "pin-off", "", false)
    } else {
        item("remote.pin", "始终保留在本地", "cloud-download", "", false)
    };
    let label = crate::remote::connection_for_path(&ctx.paths[0])
        .map(|(connection, _)| crate::i18n::t("{0} 触发同步").replace("{0}", &connection.name))
        .unwrap_or_else(|| crate::i18n::t("触发同步").into());
    if !pinned {
        activity(&mut pin, &label, "sync", true);
    }
    let mut items = vec![pin];
    if !cached {
        let mut cache = item("remote.cache", "缓存到本地", "cloud-download", "", false);
        activity(&mut cache, crate::i18n::t("正在缓存…"), "transfer", false);
        items.push(cache);
    } else if !ctx
        .entries
        .iter()
        .any(|entry| entry.pinned || matches!(entry.sync.as_deref(), Some("conflict" | "pending")))
    {
        items.push(item("remote.uncache", "取消缓存", "cloud-off", "", false));
    }
    if conflict {
        items.push(item(
            "remote.keepLocal",
            "用本地版本覆盖云端",
            "cloud-check",
            "",
            false,
        ));
        items.push(item(
            "remote.keepRemote",
            "保留本地备份并采用云端版本",
            "cloud-check",
            "",
            false,
        ));
    }
    items
}
pub(super) fn execute(id: &str, item_id: &str, ctx: &Context) -> Result<Value, String> {
    let offered = resolve(id, ctx)?;
    if !offered["items"].as_array().is_some_and(|items| {
        items
            .iter()
            .any(|item| item["id"] == item_id && item["disabled"] != true)
    }) {
        return Err("menu action is no longer available".into());
    }
    let paths = if matches!(item_id, "remote.keepLocal" | "remote.keepRemote") {
        ctx.entries
            .iter()
            .filter(|entry| entry.sync.as_deref() == Some("conflict"))
            .map(|entry| entry.path.clone())
            .collect::<Vec<_>>()
    } else {
        ctx.paths.clone()
    };
    Ok(
        json!({"kind":if item_id.starts_with("remote.") { "remoteAction" } else { "coreAction" },
        "command":item_id,"paths":paths,"path":ctx.paths[0],"directory":ctx.directory}),
    )
}
