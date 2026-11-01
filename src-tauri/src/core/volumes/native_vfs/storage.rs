//! Platform storage implementations projected through the shared VFS DTOs.
use super::{applications, recycle, trash_vfs, APPLICATIONS_URI, RECENT_URI};
use crate::{core::files::Entry, plugins::provider_registry::FsOperation};

pub(super) fn entry(value: serde_json::Value) -> Result<Entry, String> {
    crate::core::fs::presentation::entry(value)
}

pub(super) fn decorated_entry(path: &str, mut entry: Entry) -> Entry {
    let capabilities = crate::plugins::resource::file_capabilities::for_entry(path, &entry);
    entry.allowed_ops = [
        FsOperation::List,
        FsOperation::Stat,
        FsOperation::Search,
        FsOperation::Read,
        FsOperation::Write,
        FsOperation::Mkdir,
        FsOperation::Rename,
        FsOperation::Remove,
    ]
    .into_iter()
    .filter(|operation| capabilities.allows(*operation))
    .map(|operation| operation.as_str().to_owned())
    .collect();
    entry
}

pub(super) fn root_entry(
    id: &str,
    path: &str,
    readonly: bool,
    operations: &[FsOperation],
) -> Entry {
    Entry {
        path: path.into(),
        name: id.into(),
        is_dir: true,
        readonly,
        allowed_ops: operations
            .iter()
            .map(|operation| operation.as_str().to_owned())
            .collect(),
        ..Default::default()
    }
}

pub(super) fn recent_stat(path: &str, operations: &[FsOperation]) -> Result<Entry, String> {
    if path == RECENT_URI {
        Ok(root_entry("core:recent", path, true, operations))
    } else {
        crate::core::recent::entry_for_alias(path)
    }
}

pub(super) fn recent_list(path: &str) -> Result<Vec<Entry>, String> {
    if path == RECENT_URI {
        Ok(crate::core::recent::entries(
            &crate::core::recent::Recent::default(),
        ))
    } else {
        Err("Unsupported: builtin node has no children".into())
    }
}

pub(super) fn applications_stat(path: &str, operations: &[FsOperation]) -> Result<Entry, String> {
    if path == APPLICATIONS_URI {
        return Ok(root_entry("core:applications", path, false, operations));
    }
    if applications::is_user(path) {
        return applications::stored_entry(path);
    }
    if applications::is_system_app(path) {
        return applications::system_entry(path);
    }
    Err("Unsupported: unknown builtin node".into())
}

pub(super) fn applications_list(path: &str) -> Result<Vec<Entry>, String> {
    if path == APPLICATIONS_URI {
        applications::list()
    } else if applications::is_user(path) {
        applications::list_at(path)
    } else {
        Err("Unsupported: builtin node has no children".into())
    }
}

pub(super) fn trash_stat(path: &str, operations: &[FsOperation]) -> Result<Entry, String> {
    if trash_vfs::handles(path) {
        return trash_vfs::entry(path);
    }
    if path == recycle::URI {
        return Ok(root_entry("core:trash", path, true, operations));
    }
    if recycle::is_virtual(path) {
        return recycle::list()?
            .into_iter()
            .find(|entry| entry.path == path)
            .ok_or_else(|| "NotFound: Recycle Bin item missing".into());
    }
    Err("Unsupported: unknown builtin node".into())
}

pub(super) fn trash_list(path: &str) -> Result<Vec<Entry>, String> {
    if trash_vfs::handles(path) {
        trash_vfs::list(path)
    } else if path == recycle::URI {
        recycle::list()
    } else {
        Err("Unsupported: builtin node has no children".into())
    }
}
