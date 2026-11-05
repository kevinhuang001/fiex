//! Core's filesystem DTO projection for the Applications VFS adapter.
use super::super::{applications, storage, APPLICATIONS_URI};
use crate::core::files::Entry;
use fiex_provider_contracts::FsOperation;

pub(super) fn applications_stat(uri: &str, operations: &[FsOperation]) -> Result<Entry, String> {
    if uri == APPLICATIONS_URI {
        return Ok(storage::root_entry(
            fiex_applications::PROVIDER_ID,
            uri,
            operations,
        ));
    }
    if applications::is_system_app(uri) {
        return applications::system_entry(uri);
    }
    if applications::is_folder(uri) || applications::is_user(uri) {
        return applications::entry_for_uri(uri);
    }
    Err("Unsupported: unknown builtin node".into())
}

pub(super) fn applications_list(uri: &str) -> Result<Vec<Entry>, String> {
    if uri == APPLICATIONS_URI {
        applications::list()
    } else if applications::is_folder(uri) {
        applications::list_at(uri)
    } else {
        Err("Unsupported: builtin node has no children".into())
    }
}
