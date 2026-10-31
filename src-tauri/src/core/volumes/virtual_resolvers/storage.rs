//! Storage operations owned by each native VFS instance. Core routes all
//! builtin list/stat requests through the selected instance, not URI switches
//! in the generic storage adapter.
use super::{applications, recycle, trash_vfs, NativeVfsProvider, APPLICATIONS_URI, RECENT_URI};
use crate::core::{files::Entry, recent};

pub(super) fn root_entry(view: &dyn NativeVfsProvider, path: &str) -> Entry {
    Entry {
        path: path.into(),
        name: view.id().into(),
        is_dir: true,
        readonly: view.readonly(),
        allowed_ops: view
            .operations(path)
            .into_iter()
            .map(str::to_owned)
            .collect(),
        ..Default::default()
    }
}

pub(super) fn recent_stat(view: &dyn NativeVfsProvider, path: &str) -> Result<Entry, String> {
    if path == RECENT_URI {
        Ok(root_entry(view, path))
    } else {
        recent::entry_for_alias(path)
    }
}

pub(super) fn recent_list(path: &str) -> Result<Vec<Entry>, String> {
    if path == RECENT_URI {
        Ok(recent::entries(&recent::Recent::default()))
    } else {
        Err("Unsupported: builtin node has no children".into())
    }
}

pub(super) fn applications_stat(view: &dyn NativeVfsProvider, path: &str) -> Result<Entry, String> {
    if path == APPLICATIONS_URI {
        return Ok(root_entry(view, path));
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

pub(super) fn trash_stat(view: &dyn NativeVfsProvider, path: &str) -> Result<Entry, String> {
    if trash_vfs::handles(path) {
        return trash_vfs::entry(path);
    }
    if path == recycle::URI {
        return Ok(root_entry(view, path));
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
