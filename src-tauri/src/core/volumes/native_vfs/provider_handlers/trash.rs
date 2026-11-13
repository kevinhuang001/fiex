use super::shared::vfs_identity;
use crate::core::files::Entry;
use crate::core::volumes::native_vfs::{NativeVfsBehavior, NativeVfsIdentity, recycle, trash_vfs};

fn trash_open_policy(_path: &str, entry: &Entry) -> Option<bool> {
    Some(
        entry.is_dir
            && !entry.is_symlink
            && entry
                .allowed_ops
                .iter()
                .any(|operation| operation == "list"),
    )
}

#[cfg(feature = "desktop")]
fn trash_open(_path: &str) -> Result<(), String> {
    Err(crate::i18n::t("回收站项目请使用“放回原处”或“彻底删除”").into())
}

fn trash_resource_kind(path: &str) -> &'static str {
    if path == recycle::URI || path == trash_vfs::URI || path.ends_with('/') {
        "directory"
    } else {
        "file"
    }
}

pub(crate) struct TrashOperations;

impl NativeVfsBehavior for TrashOperations {
    fn validate_child(&self, parent: &str, child: &str) -> Option<Result<(), String>> {
        Some(trash_vfs::validate_child(parent, child))
    }

    fn is_tree_path(&self, path: &str) -> bool {
        trash_vfs::is_tree(path)
    }

    fn resource_kind(&self, path: &str) -> Option<&'static str> {
        Some(trash_resource_kind(path))
    }

    fn parent_path(&self, path: &str) -> Option<Result<String, String>> {
        Some(trash_vfs::parent_path(path))
    }

    fn provider_identity(
        &self,
        path: &str,
        provider_id: &'static str,
    ) -> Option<NativeVfsIdentity> {
        vfs_identity(path, provider_id)
    }

    #[cfg(feature = "desktop")]
    fn open(&self, path: &str) -> Option<Result<(), String>> {
        Some(trash_open(path))
    }

    fn open_policy(&self, path: &str, entry: &Entry) -> Option<bool> {
        trash_open_policy(path, entry)
    }
}
