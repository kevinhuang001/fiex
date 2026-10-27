//! Capability-gated builtin context-menu providers.
//!
//! The menu is a list of small resolvers. A group exists only when its
//! capability exists, so an unsupported rename/delete/new-folder group is
//! absent from the resolver list instead of appearing with an empty item array.
//! The capability matrix is:
//!
//! - `opening`: always; `open` is disabled for a file entry without `read`
//!   (a directory opens by navigation and needs no read), `preview` requires
//!   `read`, `info` only reads Core metadata.
//! - `rename` / `batch-rename`: require `rename` in a writable directory.
//! - `clipboard`: `copy` requires `read`, `cut` requires `rename` in a writable
//!   directory, `duplicate` requires `read && write` in a writable directory.
//! - `new-folder`: requires `create` and a writable directory.
//! - `trash`: requires `remove` in a writable directory; `restore`/`delete`
//!   keep today's in-trash behaviour.
//! - `terminal` / `tags`: local selections only, because Core has no remote
//!   terminal or tag path.
//! - `copy-path`: always; it only uses the location Core already knows.
//! - `connection-cache`: remote selections only, unchanged otherwise.
//!
//! Presence decides availability: an unsupported operation is absent from the
//! menu, never emitted as a disabled item. [`crate::plugins::resource::file_capabilities`]
//! re-checks the same call on execute.
use super::connection_cache::ConnectionCache;
use crate::core::files::Entry;
use crate::plugins::resource::file_capabilities::FileCapabilities;
use serde_json::{json, Value};

/// The selection a menu is resolved for.
pub(super) struct Selection<'a> {
    pub(super) paths: &'a [String],
    pub(super) entries: &'a [Entry],
    pub(super) read_only: bool,
    pub(super) in_trash: bool,
    pub(super) all_local: bool,
}

impl Selection<'_> {
    fn many(&self) -> bool {
        self.paths.len() > 1
    }
}

pub(super) trait BuiltinResolver {
    fn id(&self) -> &'static str;
    fn priority(&self) -> i64;
    fn available(&self, selection: &Selection<'_>, capabilities: &FileCapabilities) -> bool;
    fn items(&self, selection: &Selection<'_>, capabilities: &FileCapabilities) -> Vec<Value>;
}

/// Canonical order; a filtered-out group keeps every later group's priority.
const GROUPS: [&dyn BuiltinResolver; 10] = [
    &Opening,
    &Tags,
    &ConnectionCache,
    &NewFolder,
    &Rename,
    &BatchRename,
    &Clipboard,
    &Terminal,
    &CopyPath,
    &Trash,
];

pub(super) fn groups() -> &'static [&'static dyn BuiltinResolver] {
    &GROUPS
}

struct Opening;
impl BuiltinResolver for Opening {
    fn id(&self) -> &'static str {
        "builtin:opening"
    }
    fn priority(&self) -> i64 {
        100
    }
    fn available(&self, _: &Selection<'_>, _: &FileCapabilities) -> bool {
        true
    }
    fn items(&self, selection: &Selection<'_>, capabilities: &FileCapabilities) -> Vec<Value> {
        // Directories navigate without reading; a file needs `read` to open.
        let navigable = selection
            .entries
            .iter()
            .all(|entry| entry.is_dir || capabilities.read);
        let mut items = Vec::new();
        if navigable {
            items.push(item("open", "打开", "open", "open"));
        }
        if !selection.many() {
            if capabilities.read {
                items.push(item("preview", "快速查看", "eye", "quickLook"));
            }
            items.push(item("info", "显示简介", "info", "info"));
        }
        items
    }
}

struct Tags;
impl BuiltinResolver for Tags {
    fn id(&self) -> &'static str {
        "builtin:tags"
    }
    fn priority(&self) -> i64 {
        99
    }
    fn available(&self, selection: &Selection<'_>, _: &FileCapabilities) -> bool {
        selection.all_local
    }
    fn items(&self, _selection: &Selection<'_>, _: &FileCapabilities) -> Vec<Value> {
        if cfg!(target_os = "macos") {
            vec![item("tags", "标签…", "tags", "")]
        } else {
            Vec::new()
        }
    }
}

struct NewFolder;
impl BuiltinResolver for NewFolder {
    fn id(&self) -> &'static str {
        "builtin:new-folder"
    }
    fn priority(&self) -> i64 {
        97
    }
    fn available(&self, selection: &Selection<'_>, capabilities: &FileCapabilities) -> bool {
        capabilities.create && !selection.read_only
    }
    fn items(&self, _: &Selection<'_>, _: &FileCapabilities) -> Vec<Value> {
        // Offered items stay actionable; Core re-checks `mkdir` on execute.
        vec![item("newFolder", "新建文件夹", "folder-plus", "newFolder")]
    }
}

struct Rename;
impl BuiltinResolver for Rename {
    fn id(&self) -> &'static str {
        "builtin:rename"
    }
    fn priority(&self) -> i64 {
        96
    }
    fn available(&self, selection: &Selection<'_>, capabilities: &FileCapabilities) -> bool {
        capabilities.rename && !selection.read_only
    }
    fn items(&self, selection: &Selection<'_>, _: &FileCapabilities) -> Vec<Value> {
        if selection.many() {
            Vec::new()
        } else {
            vec![item("rename", "重新命名", "pencil", "rename")]
        }
    }
}

struct BatchRename;
impl BuiltinResolver for BatchRename {
    fn id(&self) -> &'static str {
        "builtin:batch-rename"
    }
    fn priority(&self) -> i64 {
        95
    }
    fn available(&self, selection: &Selection<'_>, capabilities: &FileCapabilities) -> bool {
        capabilities.rename && !selection.read_only
    }
    fn items(&self, selection: &Selection<'_>, _: &FileCapabilities) -> Vec<Value> {
        if selection.many() {
            vec![item("batchRename", "批量重命名…", "pencil", "")]
        } else {
            Vec::new()
        }
    }
}

struct Clipboard;
impl BuiltinResolver for Clipboard {
    fn id(&self) -> &'static str {
        "builtin:clipboard"
    }
    fn priority(&self) -> i64 {
        94
    }
    fn available(&self, selection: &Selection<'_>, capabilities: &FileCapabilities) -> bool {
        capabilities.read || (capabilities.rename && !selection.read_only)
    }
    fn items(&self, selection: &Selection<'_>, capabilities: &FileCapabilities) -> Vec<Value> {
        let mut items = Vec::new();
        if capabilities.read {
            items.push(item("copy", "复制", "copy", "copy"));
        }
        if capabilities.rename && !selection.read_only {
            items.push(item("cut", "剪切", "scissors", "cut"));
        }
        if capabilities.read && capabilities.write && !selection.read_only {
            items.push(item("duplicate", "创建副本", "copy", "duplicate"));
        }
        items
    }
}

struct Terminal;
impl BuiltinResolver for Terminal {
    fn id(&self) -> &'static str {
        "builtin:terminal"
    }
    fn priority(&self) -> i64 {
        93
    }
    fn available(&self, selection: &Selection<'_>, _: &FileCapabilities) -> bool {
        selection.all_local
    }
    fn items(&self, _: &Selection<'_>, _: &FileCapabilities) -> Vec<Value> {
        vec![item("terminal", "Open in Terminal", "terminal", "")]
    }
}

struct CopyPath;
impl BuiltinResolver for CopyPath {
    fn id(&self) -> &'static str {
        "builtin:copy-path"
    }
    fn priority(&self) -> i64 {
        92
    }
    fn available(&self, _: &Selection<'_>, _: &FileCapabilities) -> bool {
        true
    }
    fn items(&self, _: &Selection<'_>, _: &FileCapabilities) -> Vec<Value> {
        vec![item("copyPath", "复制路径", "copy", "")]
    }
}

struct Trash;
impl BuiltinResolver for Trash {
    fn id(&self) -> &'static str {
        "builtin:trash"
    }
    fn priority(&self) -> i64 {
        91
    }
    fn available(&self, selection: &Selection<'_>, capabilities: &FileCapabilities) -> bool {
        capabilities.remove && !selection.read_only
    }
    fn items(&self, selection: &Selection<'_>, _: &FileCapabilities) -> Vec<Value> {
        let mut items = if selection.in_trash {
            vec![
                item("restore", "放回原处", "restore", ""),
                item("delete", "彻底删除", "trash", ""),
            ]
        } else {
            vec![item("trash", "移到废纸篓", "trash", "trash")]
        };
        for item in &mut items {
            item["placement"] = json!("tail");
            item["danger"] = json!(item["id"] != "restore");
        }
        items
    }
}

pub(super) fn item(id: &str, title: &'static str, icon: &str, shortcut: &str) -> Value {
    // Availability decides presence; a builtin item is never greyed out.
    json!({"id":id,"kind":"action","title":crate::i18n::t(title),"icon":icon,"shortcut":shortcut,"disabled":false})
}
