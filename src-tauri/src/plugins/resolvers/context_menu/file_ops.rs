//! Capability-gated builtin context-menu providers.
//!
//! The menu is a list of small resolvers. A group exists only when its
//! capability exists, so an unsupported rename/delete group is absent from the
//! resolver list instead of appearing with an empty item array.
//! The capability matrix is:
//!
//! - `opening`: always; `open` is omitted for a file entry without `read`
//!   (a directory opens by navigation and needs no read), `preview` requires
//!   `read`, `info` only reads Core metadata.
//! - `rename` / `batch-rename`: require `rename` in a writable directory.
//! - `clipboard`: `copy` requires `read`, `cut` requires `rename` in a writable
//!   directory, `duplicate` requires `read && write` in a writable directory.
//! - `trash`: requires `remove` in a writable directory. View-specific
//!   restore, purge and empty actions belong only to the registered view provider.
//! - `terminal` / `tags`: local selections only, because Core has no remote
//!   terminal or tag path.
//! - `copy-path`: always; it only uses the location Core already knows.
//! - `connection-cache`: remote selections only, unchanged otherwise.
//!
//! Folder creation is not a menu group: the toolbar owns the single
//! "new folder" affordance and disables it from the listing's `canCreate`
//! (provider `mkdir` ∩ the directory node's own restrictions).
//!
//! Presence decides availability: an unsupported operation is absent from the
//! menu, never emitted as a disabled item. [`crate::plugins::resource::file_capabilities`]
//! re-checks the same call on execute.
use super::connection_cache::ConnectionCache;
use crate::core::files::Entry;
use crate::plugins::provider_registry::native::NativeProviderRegistry;
use crate::plugins::resource::file_capabilities::FileCapabilities;
use fiex_provider_contracts::{ProviderKind, ProviderMatchRule};
use serde_json::{json, Value};
use std::sync::{Arc, OnceLock};

/// The selection a menu is resolved for.
pub(super) struct Selection<'a> {
    pub(super) paths: &'a [String],
    pub(super) entries: &'a [Entry],
    pub(super) read_only: bool,
    pub(super) all_local: bool,
}

impl Selection<'_> {
    fn many(&self) -> bool {
        self.paths.len() > 1
    }
}

pub(super) trait BuiltinResolver: Send + Sync {
    fn id(&self) -> &'static str;
    fn priority(&self) -> i64;
    fn available(&self, selection: &Selection<'_>, capabilities: &FileCapabilities) -> bool;
    fn items(&self, selection: &Selection<'_>, capabilities: &FileCapabilities) -> Vec<Value>;
}

/// Every generic Core menu group is a registered typed provider instance.
/// Capabilities determine eligibility; no per-request provider ID switch.
fn registry() -> &'static NativeProviderRegistry<dyn BuiltinResolver> {
    static REGISTERED: OnceLock<NativeProviderRegistry<dyn BuiltinResolver>> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let mut values: NativeProviderRegistry<dyn BuiltinResolver> =
            NativeProviderRegistry::new(ProviderKind::ContextMenu);
        let implementations: [Arc<dyn BuiltinResolver>; 10] = [
            Arc::new(Opening),
            Arc::new(Tags),
            Arc::new(Create),
            Arc::new(ConnectionCache),
            Arc::new(Rename),
            Arc::new(BatchRename),
            Arc::new(Clipboard),
            Arc::new(Terminal),
            Arc::new(CopyPath),
            Arc::new(Trash),
        ];
        for implementation in implementations {
            values
                .register(
                    implementation.id(),
                    implementation.priority(),
                    ProviderMatchRule::default(),
                    implementation,
                )
                .expect("invalid builtin menu registration");
        }
        values
    })
}

pub(super) fn available(
    selection: &Selection<'_>,
    capabilities: &FileCapabilities,
) -> Vec<&'static dyn BuiltinResolver> {
    registry()
        .candidates(|implementation| implementation.available(selection, capabilities))
        .iter()
        .filter_map(|candidate| registry().get(&candidate.id))
        .collect()
}

struct Opening;
impl BuiltinResolver for Opening {
    fn id(&self) -> &'static str {
        "builtin:opening"
    }
    fn priority(&self) -> i64 {
        100
    }
    fn available(&self, selection: &Selection<'_>, _: &FileCapabilities) -> bool {
        !selection.paths.is_empty()
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
        selection.all_local && !selection.paths.is_empty()
    }
    fn items(&self, _selection: &Selection<'_>, _: &FileCapabilities) -> Vec<Value> {
        if cfg!(target_os = "macos") {
            vec![item("tags", "标签…", "tags", "")]
        } else {
            Vec::new()
        }
    }
}

/// Blank-canvas actions: there is no selection, so only creating entries is
/// meaningful. Both items are capability-gated: `mkdir` for a folder and the
/// provider's `write` (parentId + name) for an empty file.
struct Create;
impl BuiltinResolver for Create {
    fn id(&self) -> &'static str {
        "builtin:create"
    }
    fn priority(&self) -> i64 {
        98
    }
    fn available(&self, selection: &Selection<'_>, capabilities: &FileCapabilities) -> bool {
        selection.paths.is_empty() && (capabilities.create || capabilities.write)
    }
    fn items(&self, _selection: &Selection<'_>, capabilities: &FileCapabilities) -> Vec<Value> {
        let mut items = Vec::new();
        if capabilities.create {
            items.push(item("newFolder", "新建文件夹…", "folder-plus", "newFolder"));
        }
        if capabilities.write {
            items.push(item("newFile", "新建空文件…", "file-plus", "newFile"));
        }
        items
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
        capabilities.rename && !selection.read_only && !selection.paths.is_empty()
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
        capabilities.rename && !selection.read_only && !selection.paths.is_empty()
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
        !selection.paths.is_empty()
            && (capabilities.read
                || (capabilities.read && capabilities.remove && !selection.read_only))
    }
    fn items(&self, selection: &Selection<'_>, capabilities: &FileCapabilities) -> Vec<Value> {
        let mut items = Vec::new();
        if capabilities.read {
            items.push(item("copy", "复制", "copy", "copy"));
        }
        if capabilities.read && capabilities.remove && !selection.read_only {
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
        selection.all_local && !selection.paths.is_empty()
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
    fn available(&self, selection: &Selection<'_>, _: &FileCapabilities) -> bool {
        !selection.paths.is_empty()
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
        capabilities.remove && !selection.read_only && !selection.paths.is_empty()
    }
    fn items(&self, selection: &Selection<'_>, _: &FileCapabilities) -> Vec<Value> {
        let mut items = if selection.all_local {
            // The platform Trash only accepts local paths. Windows exposes the
            // Recycle Bin under that name; the rest keep the Trash wording.
            let title = if cfg!(windows) {
                "移到回收站"
            } else {
                "移到废纸篓"
            };
            vec![item("trash", title, "trash", "trash")]
        } else {
            // Provider-backed mounts have no Trash. Their `vfs.remove` is the
            // permanent delete, so the menu says exactly that.
            vec![item("delete", "彻底删除", "trash", "")]
        };
        for item in &mut items {
            item["placement"] = json!("tail");
            item["danger"] = json!(true);
        }
        items
    }
}

pub(super) fn item(id: &str, title: &'static str, icon: &str, shortcut: &str) -> Value {
    // Availability decides presence; a builtin item is never greyed out.
    json!({"id":id,"kind":"action","title":crate::i18n::t(title),"icon":icon,"shortcut":shortcut,"disabled":false})
}

#[cfg(test)]
mod registry_tests {
    use super::*;

    #[test]
    fn generic_native_menus_are_distinct_registered_context_menu_providers() {
        let registered = registry();
        assert_eq!(registered.len(), 10);
        for descriptor in registered.descriptors() {
            assert_eq!(descriptor.kind, ProviderKind::ContextMenu);
            descriptor.validate().unwrap();
        }
        let sorted = registered.candidates(|_| true);
        assert_eq!(sorted[0].id, "builtin:opening");
        assert_eq!(sorted.last().unwrap().id, "builtin:trash");
        assert!(sorted
            .windows(2)
            .all(|pair| pair[0].priority >= pair[1].priority));
        assert!(registered.get("builtin:nonexistent").is_none());
    }

    #[test]
    fn capabilities_are_checked_by_each_registered_native_menu() {
        let no_paths: Vec<String> = Vec::new();
        let entries: Vec<Entry> = Vec::new();
        let selection = Selection {
            paths: &no_paths,
            entries: &entries,
            read_only: false,
            all_local: true,
        };
        assert!(available(&selection, &FileCapabilities::default()).is_empty());
        let allowed = available(&selection, &FileCapabilities::ALL);
        assert_eq!(allowed.len(), 1);
        assert_eq!(allowed[0].id(), "builtin:create");
    }
}
